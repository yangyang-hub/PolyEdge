const REWARD_REPLAY_CAPTURE_QUEUE_CAPACITY: usize = 2;

struct RewardReplayCaptureJob {
    run_id: i64,
    fixture: polyedge_application::RewardDecisionReplayFixture,
    captured_at: OffsetDateTime,
    enqueued_at: Instant,
    trace_id: String,
    expected_plans: Vec<RewardQuotePlan>,
}

enum RewardReplayCaptureMessage {
    Capture(Box<RewardReplayCaptureJob>),
    Shutdown(tokio::sync::oneshot::Sender<()>),
}

static REWARD_REPLAY_CAPTURE_TX: Mutex<Option<mpsc::Sender<RewardReplayCaptureMessage>>> =
    Mutex::new(None);

struct RewardReplayCaptureRuntime {
    sender: mpsc::Sender<RewardReplayCaptureMessage>,
    handle: JoinHandle<()>,
}

impl RewardReplayCaptureRuntime {
    fn start(state: &AppState) -> Self {
        let (sender, receiver) = mpsc::channel(REWARD_REPLAY_CAPTURE_QUEUE_CAPACITY);
        let service = state.reward_bot_service.clone();
        let handle = tokio::spawn(run_reward_replay_capture_writer(service, receiver));
        let mut current = REWARD_REPLAY_CAPTURE_TX
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        if current.replace(sender.clone()).is_some() {
            warn!("replaced an existing rewards replay capture writer sender");
        }
        Self { sender, handle }
    }

    async fn shutdown(self) {
        {
            let mut current = REWARD_REPLAY_CAPTURE_TX
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            if current
                .as_ref()
                .is_some_and(|sender| sender.same_channel(&self.sender))
            {
                current.take();
            }
        }
        let (ack_tx, ack_rx) = tokio::sync::oneshot::channel();
        let shutdown_sent = tokio::time::timeout(
            Duration::from_secs(5),
            self.sender.send(RewardReplayCaptureMessage::Shutdown(ack_tx)),
        )
        .await
        .is_ok_and(|result| result.is_ok());
        if !shutdown_sent
            || tokio::time::timeout(Duration::from_secs(5), ack_rx)
                .await
                .is_err()
        {
            warn!("timed out draining rewards replay capture writer during shutdown");
            self.handle.abort();
        }
        if let Err(error) = self.handle.await
            && !error.is_cancelled()
        {
            warn!(error = %error, "rewards replay capture writer failed to join");
        }
    }
}

fn enqueue_reward_replay_fixture(
    run_id: i64,
    input: RewardStrategyInput,
    cycle: &RewardLiveCycle,
    books: &HashMap<String, RewardOrderBook>,
    book_history: &HashMap<String, VecDeque<BookSnapshot>>,
    trace_id: &str,
) {
    let sender = {
        REWARD_REPLAY_CAPTURE_TX
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .clone()
    };
    let Some(sender) = sender else {
        warn!(
            trace_id = %trace_id,
            run_id,
            capture_status = "writer_not_started",
            "skipped rewards replay capture because writer runtime is unavailable"
        );
        return;
    };
    let permit = match sender.try_reserve() {
        Ok(permit) => permit,
        Err(mpsc::error::TrySendError::Full(_)) => {
            warn!(
                trace_id = %trace_id,
                run_id,
                capture_status = "queue_full",
                queue_capacity = REWARD_REPLAY_CAPTURE_QUEUE_CAPACITY,
                "dropped rewards replay capture without blocking live tick"
            );
            return;
        }
        Err(mpsc::error::TrySendError::Closed(_)) => {
            warn!(
                trace_id = %trace_id,
                run_id,
                capture_status = "writer_closed",
                "skipped rewards replay capture because writer is unavailable"
            );
            return;
        }
    };

    let providers = RewardReplayProviderSnapshot {
        advisories: cycle
            .plans
            .iter()
            .filter_map(|plan| {
                plan.ai_advisory
                    .clone()
                    .map(|advisory| (plan.condition_id.clone(), advisory))
            })
            .collect(),
        info_risks: cycle
            .plans
            .iter()
            .filter_map(|plan| {
                plan.info_risk
                    .clone()
                    .map(|risk| (plan.condition_id.clone(), risk))
            })
            .collect(),
    };
    let started = Instant::now();
    let fixture = build_reward_decision_replay_fixture_v2_pending_expectations(
        input,
        providers,
        &cycle.account,
        &cycle.open_orders,
        &cycle.positions,
        books,
        book_history,
    );
    let expected_plans = cycle.plans.clone();
    let build_ms = u64::try_from(started.elapsed().as_millis()).unwrap_or(u64::MAX);
    permit.send(RewardReplayCaptureMessage::Capture(Box::new(RewardReplayCaptureJob {
        run_id,
        fixture,
        captured_at: OffsetDateTime::now_utc(),
        enqueued_at: Instant::now(),
        trace_id: trace_id.to_string(),
        expected_plans,
    })));
    debug!(
        trace_id = %trace_id,
        run_id,
        capture_status = "queued",
        build_ms,
        queue_depth = REWARD_REPLAY_CAPTURE_QUEUE_CAPACITY.saturating_sub(sender.capacity()),
        queue_capacity = REWARD_REPLAY_CAPTURE_QUEUE_CAPACITY,
        "queued compact rewards replay fixture"
    );
}

async fn run_reward_replay_capture_writer(
    service: Arc<polyedge_application::RewardBotService>,
    mut receiver: mpsc::Receiver<RewardReplayCaptureMessage>,
) {
    while let Some(message) = receiver.recv().await {
        let RewardReplayCaptureMessage::Capture(job) = message else {
            if let RewardReplayCaptureMessage::Shutdown(ack) = message {
                let _ = ack.send(());
            }
            break;
        };
        let job = *job;
        let trace_id = job.trace_id.clone();
        let run_id = job.run_id;
        let captured_at = job.captured_at;
        let queue_wait_ms = u64::try_from(job.enqueued_at.elapsed().as_millis()).unwrap_or(u64::MAX);
        let capture_started = Instant::now();
        let fixture = tokio::task::spawn_blocking(move || {
            let mut fixture = job.fixture;
            set_reward_replay_expected_plan_hashes(&mut fixture, &job.expected_plans)?;
            Ok::<_, AppError>(fixture)
        })
        .await;
        let capture_ms = u64::try_from(capture_started.elapsed().as_millis()).unwrap_or(u64::MAX);
        let fixture = match fixture {
            Ok(Ok(fixture)) => fixture,
            Ok(Err(error)) => {
                warn!(
                    trace_id = %trace_id,
                    run_id,
                    capture_status = "capture_rejected",
                    queue_wait_ms,
                    capture_ms,
                    error = %error,
                    "skipped unsafe or oversized rewards replay fixture"
                );
                continue;
            }
            Err(error) => {
                warn!(
                    trace_id = %trace_id,
                    run_id,
                    capture_status = "worker_failed",
                    queue_wait_ms,
                    capture_ms,
                    error = %error,
                    "rewards replay capture blocking task failed"
                );
                continue;
            }
        };

        let persist_started = Instant::now();
        let record = match service
            .capture_and_save_strategy_replay_fixture(run_id, fixture, captured_at)
            .await
        {
            Ok(record) => record,
            Err(error) => {
                warn!(
                    trace_id = %trace_id,
                    run_id,
                    capture_status = "persist_failed",
                    queue_wait_ms,
                    capture_ms,
                    persist_ms = u64::try_from(persist_started.elapsed().as_millis()).unwrap_or(u64::MAX),
                    error = %error,
                    "failed to capture or persist rewards replay fixture"
                );
                continue;
            }
        };
        let json_bytes = record.json_bytes;
        info!(
            trace_id = %trace_id,
            run_id,
            capture_status = "persisted",
            schema_version = REWARD_DECISION_REPLAY_SCHEMA_VERSION,
            json_bytes,
            queue_wait_ms,
            capture_ms,
            persist_ms = u64::try_from(persist_started.elapsed().as_millis()).unwrap_or(u64::MAX),
            "persisted compact rewards replay fixture"
        );
    }
}
