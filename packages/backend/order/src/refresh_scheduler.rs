use crate::metrics::OrderbookRuntimeMetrics;
use polyedge_connectors::{PolymarketRewardOrderBook, PolymarketRewardsConnector};
use polyedge_domain::AppError;
use std::array;
use std::collections::{HashSet, VecDeque};
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::{mpsc, oneshot};
use tokio::time::Instant;

const REFRESH_QUEUE_CAPACITY: usize = 128;
pub(crate) const REFRESH_GATE_WAIT_TIMEOUT: Duration = Duration::from_secs(2);
pub(crate) const REFRESH_UPSTREAM_TIMEOUT: Duration = Duration::from_secs(8);
const PRIORITY_SCHEDULE: [RefreshPriority; 15] = [
    RefreshPriority::LiveAction,
    RefreshPriority::LiveAction,
    RefreshPriority::LiveAction,
    RefreshPriority::LiveAction,
    RefreshPriority::LiveAction,
    RefreshPriority::LiveAction,
    RefreshPriority::LiveAction,
    RefreshPriority::LiveAction,
    RefreshPriority::HttpRefresh,
    RefreshPriority::HttpRefresh,
    RefreshPriority::HttpRefresh,
    RefreshPriority::HttpRefresh,
    RefreshPriority::BackgroundActive,
    RefreshPriority::BackgroundActive,
    RefreshPriority::CandidatePrewarm,
];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum RefreshPriority {
    LiveAction = 0,
    HttpRefresh = 1,
    BackgroundActive = 2,
    CandidatePrewarm = 3,
}

impl RefreshPriority {
    const COUNT: usize = 4;

    fn index(self) -> usize {
        self as usize
    }
}

#[derive(Clone)]
pub(crate) enum RefreshBatchOutcome {
    Completed(Vec<PolymarketRewardOrderBook>),
    Deferred,
    Failed(AppError),
}

struct RefreshCommand {
    priority: RefreshPriority,
    token_ids: Vec<String>,
    queued_at: Instant,
    response_txs: Vec<oneshot::Sender<RefreshBatchOutcome>>,
}

#[derive(Clone)]
pub(crate) struct OrderbookRefreshScheduler {
    command_tx: mpsc::Sender<RefreshCommand>,
    metrics: Arc<OrderbookRuntimeMetrics>,
}

impl OrderbookRefreshScheduler {
    pub(crate) fn spawn(
        connector: PolymarketRewardsConnector,
        metrics: Arc<OrderbookRuntimeMetrics>,
    ) -> Self {
        let (command_tx, command_rx) = mpsc::channel(REFRESH_QUEUE_CAPACITY);
        tokio::spawn(run_scheduler(connector, Arc::clone(&metrics), command_rx));
        Self {
            command_tx,
            metrics,
        }
    }

    pub(crate) async fn refresh(
        &self,
        priority: RefreshPriority,
        token_ids: &[String],
    ) -> RefreshBatchOutcome {
        let token_ids = normalized_token_ids(token_ids);
        if token_ids.is_empty() {
            return RefreshBatchOutcome::Completed(Vec::new());
        }

        let (response_tx, response_rx) = oneshot::channel();
        let command = RefreshCommand {
            priority,
            token_ids,
            queued_at: Instant::now(),
            response_txs: vec![response_tx],
        };
        self.metrics.increment_refresh_queued(priority.index());
        if self.command_tx.try_send(command).is_err() {
            self.metrics.decrement_refresh_queued(priority.index());
            self.metrics.increment_refresh_deferred();
            return RefreshBatchOutcome::Deferred;
        }
        match response_rx.await {
            Ok(outcome) => outcome,
            Err(_) => RefreshBatchOutcome::Failed(AppError::dependency_unavailable(
                "ORDERBOOK_REFRESH_SCHEDULER_STOPPED",
                "orderbook refresh scheduler stopped before completing the request",
            )),
        }
    }
}

async fn run_scheduler(
    connector: PolymarketRewardsConnector,
    metrics: Arc<OrderbookRuntimeMetrics>,
    mut command_rx: mpsc::Receiver<RefreshCommand>,
) {
    let mut queues: [VecDeque<RefreshCommand>; RefreshPriority::COUNT] =
        array::from_fn(|_| VecDeque::new());
    let mut schedule_cursor = 0usize;
    let mut in_flight: Option<InFlightRefresh> = None;
    let mut command_channel_closed = false;

    loop {
        if in_flight.is_none() {
            if queues.iter().all(VecDeque::is_empty) && !command_channel_closed {
                match command_rx.recv().await {
                    Some(command) => enqueue_command(&mut queues, command, &metrics),
                    None => command_channel_closed = true,
                }
            }
            while let Ok(command) = command_rx.try_recv() {
                enqueue_command(&mut queues, command, &metrics);
            }
            if let Some(command) = pop_weighted(&mut queues, &mut schedule_cursor) {
                metrics.decrement_refresh_queued(command.priority.index());
                let queue_wait = command.queued_at.elapsed();
                metrics.observe_refresh_queue_wait(queue_wait);
                if queue_wait > REFRESH_GATE_WAIT_TIMEOUT {
                    metrics.increment_refresh_deferred();
                    fan_out(command.response_txs, RefreshBatchOutcome::Deferred);
                    continue;
                }
                in_flight = Some(start_refresh(
                    connector.clone(),
                    Arc::clone(&metrics),
                    command,
                ));
            } else if command_channel_closed {
                return;
            } else {
                continue;
            }
        }

        let Some(mut current) = in_flight.take() else {
            continue;
        };
        tokio::select! {
            biased;
            result = &mut current.handle => {
                let outcome = result.unwrap_or_else(|error| {
                    metrics.increment_refresh_failed();
                    RefreshBatchOutcome::Failed(AppError::internal(
                        "ORDERBOOK_REFRESH_TASK_FAILED",
                        format!("orderbook refresh task failed: {error}"),
                    ))
                });
                fan_out(current.response_txs, outcome);
            }
            command = command_rx.recv(), if !command_channel_closed => {
                match command {
                    Some(command) if command.token_ids == current.token_ids => {
                        metrics.decrement_refresh_queued(command.priority.index());
                        metrics.increment_refresh_coalesced();
                        current.response_txs.extend(command.response_txs);
                    }
                    Some(command) => enqueue_command(&mut queues, command, &metrics),
                    None => command_channel_closed = true,
                }
                in_flight = Some(current);
            }
        }
    }
}

struct InFlightRefresh {
    token_ids: Vec<String>,
    response_txs: Vec<oneshot::Sender<RefreshBatchOutcome>>,
    handle: tokio::task::JoinHandle<RefreshBatchOutcome>,
}

fn start_refresh(
    connector: PolymarketRewardsConnector,
    metrics: Arc<OrderbookRuntimeMetrics>,
    command: RefreshCommand,
) -> InFlightRefresh {
    let token_ids = command.token_ids;
    let fetch_tokens = token_ids.clone();
    let handle = tokio::spawn(async move {
        let started_at = Instant::now();
        let outcome = match tokio::time::timeout(
            REFRESH_UPSTREAM_TIMEOUT,
            connector.fetch_order_books(&fetch_tokens),
        )
        .await
        {
            Ok(Ok(books)) => {
                metrics.increment_refresh_succeeded();
                RefreshBatchOutcome::Completed(books)
            }
            Ok(Err(error)) => {
                metrics.increment_refresh_failed();
                RefreshBatchOutcome::Failed(error)
            }
            Err(_) => {
                metrics.increment_refresh_failed();
                RefreshBatchOutcome::Failed(AppError::dependency_unavailable(
                    "ORDERBOOK_REFRESH_UPSTREAM_TIMEOUT",
                    format!(
                        "orderbook upstream refresh exceeded {} seconds",
                        REFRESH_UPSTREAM_TIMEOUT.as_secs()
                    ),
                ))
            }
        };
        metrics.observe_refresh_upstream_duration(started_at.elapsed());
        outcome
    });
    InFlightRefresh {
        token_ids,
        response_txs: command.response_txs,
        handle,
    }
}

fn normalized_token_ids(token_ids: &[String]) -> Vec<String> {
    let mut seen = HashSet::new();
    let mut normalized = token_ids
        .iter()
        .map(|token_id| token_id.trim())
        .filter(|token_id| !token_id.is_empty() && seen.insert((*token_id).to_string()))
        .map(ToString::to_string)
        .collect::<Vec<_>>();
    normalized.sort_unstable();
    normalized
}

fn enqueue_command(
    queues: &mut [VecDeque<RefreshCommand>; RefreshPriority::COUNT],
    mut command: RefreshCommand,
    metrics: &OrderbookRuntimeMetrics,
) {
    for queue_index in 0..queues.len() {
        if let Some(position) = queues[queue_index]
            .iter()
            .position(|existing| existing.token_ids == command.token_ids)
        {
            let Some(mut existing) = queues[queue_index].remove(position) else {
                continue;
            };
            metrics.decrement_refresh_queued(command.priority.index());
            metrics.increment_refresh_coalesced();
            existing.response_txs.append(&mut command.response_txs);
            let target_priority = if command.priority.index() < existing.priority.index() {
                metrics.decrement_refresh_queued(existing.priority.index());
                metrics.increment_refresh_queued(command.priority.index());
                existing.queued_at = command.queued_at;
                command.priority
            } else {
                existing.queued_at = existing.queued_at.min(command.queued_at);
                existing.priority
            };
            existing.priority = target_priority;
            queues[target_priority.index()].push_back(existing);
            return;
        }
    }
    queues[command.priority.index()].push_back(command);
}

fn fan_out(response_txs: Vec<oneshot::Sender<RefreshBatchOutcome>>, outcome: RefreshBatchOutcome) {
    for response_tx in response_txs {
        let _ = response_tx.send(outcome.clone());
    }
}

fn pop_weighted(
    queues: &mut [VecDeque<RefreshCommand>; RefreshPriority::COUNT],
    cursor: &mut usize,
) -> Option<RefreshCommand> {
    for _ in 0..PRIORITY_SCHEDULE.len() {
        let priority = PRIORITY_SCHEDULE[*cursor];
        *cursor = (*cursor + 1) % PRIORITY_SCHEDULE.len();
        if let Some(command) = queues[priority.index()].pop_front() {
            return Some(command);
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    fn command(priority: RefreshPriority) -> RefreshCommand {
        let (response_tx, _response_rx) = oneshot::channel();
        RefreshCommand {
            priority,
            token_ids: vec!["1".to_string()],
            queued_at: Instant::now(),
            response_txs: vec![response_tx],
        }
    }

    #[test]
    fn weighted_queue_eventually_services_candidate_work() {
        let mut queues: [VecDeque<RefreshCommand>; RefreshPriority::COUNT] =
            array::from_fn(|_| VecDeque::new());
        for _ in 0..20 {
            queues[RefreshPriority::LiveAction.index()]
                .push_back(command(RefreshPriority::LiveAction));
        }
        queues[RefreshPriority::CandidatePrewarm.index()]
            .push_back(command(RefreshPriority::CandidatePrewarm));
        let mut cursor = 0;
        let serviced = (0..PRIORITY_SCHEDULE.len())
            .filter_map(|_| pop_weighted(&mut queues, &mut cursor))
            .map(|command| command.priority)
            .collect::<Vec<_>>();
        assert!(serviced.contains(&RefreshPriority::CandidatePrewarm));
    }

    #[test]
    fn token_sets_are_trimmed_deduplicated_and_order_independent() {
        let normalized = normalized_token_ids(&[
            " 2 ".to_string(),
            "1".to_string(),
            "2".to_string(),
            "".to_string(),
        ]);
        assert_eq!(normalized, vec!["1".to_string(), "2".to_string()]);
    }

    #[test]
    fn identical_queued_requests_are_coalesced() {
        let metrics = OrderbookRuntimeMetrics::default();
        let mut queues: [VecDeque<RefreshCommand>; RefreshPriority::COUNT] =
            array::from_fn(|_| VecDeque::new());
        metrics.increment_refresh_queued(RefreshPriority::HttpRefresh.index());
        enqueue_command(&mut queues, command(RefreshPriority::HttpRefresh), &metrics);
        metrics.increment_refresh_queued(RefreshPriority::BackgroundActive.index());
        enqueue_command(
            &mut queues,
            command(RefreshPriority::BackgroundActive),
            &metrics,
        );
        assert_eq!(
            queues[RefreshPriority::HttpRefresh.index()][0]
                .response_txs
                .len(),
            2
        );
        assert_eq!(metrics.snapshot().refresh_coalesced, 1);
    }

    #[test]
    fn coalesced_request_is_promoted_to_higher_priority() {
        let metrics = OrderbookRuntimeMetrics::default();
        let mut queues: [VecDeque<RefreshCommand>; RefreshPriority::COUNT] =
            array::from_fn(|_| VecDeque::new());
        metrics.increment_refresh_queued(RefreshPriority::CandidatePrewarm.index());
        enqueue_command(
            &mut queues,
            command(RefreshPriority::CandidatePrewarm),
            &metrics,
        );
        metrics.increment_refresh_queued(RefreshPriority::LiveAction.index());
        enqueue_command(&mut queues, command(RefreshPriority::LiveAction), &metrics);
        assert!(queues[RefreshPriority::CandidatePrewarm.index()].is_empty());
        assert_eq!(
            queues[RefreshPriority::LiveAction.index()][0]
                .response_txs
                .len(),
            2
        );
    }
}
