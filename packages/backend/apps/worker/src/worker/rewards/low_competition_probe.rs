const REWARD_LOW_COMPETITION_PROBE_SOURCE: &str = "rewards_low_competition_probe";
const REWARD_LOW_COMPETITION_PROBE_MARKETS_PER_BATCH: usize = 10;
const REWARD_LOW_COMPETITION_PROBE_MAX_BATCH_SECS: i64 = 300;

#[derive(Debug, Default)]
struct LowCompetitionProbeState {
    current_conditions: Vec<String>,
    current_token_ids: Vec<String>,
    started_at: Option<OffsetDateTime>,
}

impl LowCompetitionProbeState {
    fn reset(&mut self) {
        self.current_conditions.clear();
        self.current_token_ids.clear();
        self.started_at = None;
    }

    async fn refresh_registration(
        &mut self,
        state: &AppState,
        candidates: &[RewardCandidateMarket],
        book_history: &HashMap<String, VecDeque<BookSnapshot>>,
        trace_id: &str,
    ) -> Result<()> {
        let config = state.reward_bot_service.read_config().await?;
        let low_candidates = low_competition_probe_candidates(candidates);
        if !config.low_competition_mode.is_enabled()
            || config.low_competition_max_markets == 0
            || config.low_competition_max_open_orders == 0
            || low_candidates.is_empty()
        {
            self.clear_remote_registration(state, trace_id).await?;
            return Ok(());
        }

        let now = OffsetDateTime::now_utc();
        let current_is_valid = low_competition_probe_batch_is_valid(
            &self.current_conditions,
            &low_candidates,
        );
        let should_rotate = self.current_conditions.is_empty()
            || !current_is_valid
            || self.batch_timed_out(now)
            || self.batch_is_ready(&low_candidates, book_history, &config);

        if should_rotate {
            self.current_conditions =
                select_low_competition_probe_conditions(&low_candidates, &self.current_conditions);
            self.started_at = Some(now);
            self.current_token_ids =
                low_competition_probe_token_ids(&low_candidates, &self.current_conditions);
        }

        if let Err(error) = state
            .orderbook_registry
            .register_tokens(REWARD_LOW_COMPETITION_PROBE_SOURCE, &self.current_token_ids)
            .await
        {
            warn!(
                trace_id = %trace_id,
                source = REWARD_LOW_COMPETITION_PROBE_SOURCE,
                error = %error,
                "failed to register low-competition probe orderbook batch",
            );
            return Ok(());
        }
        if should_rotate || !self.current_token_ids.is_empty() {
            info!(
                trace_id = %trace_id,
                source = REWARD_LOW_COMPETITION_PROBE_SOURCE,
                markets = self.current_conditions.len(),
                tokens = self.current_token_ids.len(),
                rotated = should_rotate,
                "registered low-competition probe orderbook batch",
            );
        }
        Ok(())
    }

    async fn clear_remote_registration(&mut self, state: &AppState, trace_id: &str) -> Result<()> {
        let had_local_batch =
            !self.current_conditions.is_empty() || !self.current_token_ids.is_empty();
        self.reset();
        if let Err(error) = state
            .orderbook_registry
            .register_tokens(REWARD_LOW_COMPETITION_PROBE_SOURCE, &[])
            .await
        {
            warn!(
                trace_id = %trace_id,
                source = REWARD_LOW_COMPETITION_PROBE_SOURCE,
                error = %error,
                "failed to clear low-competition probe orderbook batch",
            );
            return Ok(());
        }
        if had_local_batch {
            info!(
                trace_id = %trace_id,
                source = REWARD_LOW_COMPETITION_PROBE_SOURCE,
                "cleared low-competition probe orderbook batch",
            );
        }
        Ok(())
    }

    fn batch_timed_out(&self, now: OffsetDateTime) -> bool {
        self.started_at.is_some_and(|started_at| {
            now - started_at >= TimeDuration::seconds(REWARD_LOW_COMPETITION_PROBE_MAX_BATCH_SECS)
        })
    }

    fn batch_is_ready(
        &self,
        low_candidates: &[&RewardCandidateMarket],
        book_history: &HashMap<String, VecDeque<BookSnapshot>>,
        config: &RewardBotConfig,
    ) -> bool {
        !self.current_conditions.is_empty()
            && self.current_conditions.iter().all(|condition_id| {
                low_candidates
                    .iter()
                    .find(|candidate| candidate.market.condition_id == *condition_id)
                    .is_some_and(|candidate| {
                        let token_ids =
                            select_reward_book_token_ids(std::slice::from_ref(&candidate.market));
                        !token_ids.is_empty()
                            && token_ids.iter().all(|token_id| {
                                low_competition_probe_token_history_is_ready(
                                    token_id,
                                    book_history,
                                    config,
                                )
                            })
                    })
            })
    }
}

fn low_competition_probe_candidates(
    candidates: &[RewardCandidateMarket],
) -> Vec<&RewardCandidateMarket> {
    candidates
        .iter()
        .filter(|candidate| candidate.strategy_bucket == RewardStrategyBucket::LowCompetition)
        .collect()
}

fn low_competition_probe_batch_is_valid(
    current_conditions: &[String],
    low_candidates: &[&RewardCandidateMarket],
) -> bool {
    current_conditions.iter().all(|condition_id| {
        low_candidates
            .iter()
            .any(|candidate| candidate.market.condition_id == *condition_id)
    })
}

fn select_low_competition_probe_conditions(
    low_candidates: &[&RewardCandidateMarket],
    current_conditions: &[String],
) -> Vec<String> {
    if low_candidates.is_empty() {
        return Vec::new();
    }
    let start_index = current_conditions
        .last()
        .and_then(|condition_id| {
            low_candidates
                .iter()
                .position(|candidate| candidate.market.condition_id == *condition_id)
        })
        .map_or(0, |index| (index + 1) % low_candidates.len());

    let mut selected = Vec::new();
    for offset in 0..low_candidates.len() {
        if selected.len() >= REWARD_LOW_COMPETITION_PROBE_MARKETS_PER_BATCH {
            break;
        }
        let index = (start_index + offset) % low_candidates.len();
        let condition_id = low_candidates[index].market.condition_id.trim();
        if condition_id.is_empty() {
            continue;
        }
        selected.push(condition_id.to_string());
    }
    selected
}

fn low_competition_probe_token_ids(
    low_candidates: &[&RewardCandidateMarket],
    condition_ids: &[String],
) -> Vec<String> {
    let mut markets = Vec::new();
    for condition_id in condition_ids {
        if let Some(candidate) = low_candidates
            .iter()
            .find(|candidate| candidate.market.condition_id == *condition_id)
        {
            markets.push(candidate.market.clone());
        }
    }
    select_reward_book_token_ids(&markets)
}

fn low_competition_probe_token_history_is_ready(
    token_id: &str,
    book_history: &HashMap<String, VecDeque<BookSnapshot>>,
    config: &RewardBotConfig,
) -> bool {
    let Some(history) = book_history.get(token_id) else {
        return false;
    };
    let min_samples = usize::try_from(config.low_competition_min_book_samples).unwrap_or(usize::MAX);
    if history.len() < min_samples {
        return false;
    }
    if config.stale_book_ms == 0 {
        return true;
    }
    let Some(snapshot) = history.back() else {
        return false;
    };
    let max_age_ms = i64::try_from(config.stale_book_ms).unwrap_or(i64::MAX);
    let now = OffsetDateTime::now_utc();
    snapshot.observed_at <= now
        && now - snapshot.observed_at <= TimeDuration::milliseconds(max_age_ms)
}

#[cfg(test)]
mod low_competition_probe_tests {
    use super::*;

    fn probe_candidate(index: usize) -> RewardCandidateMarket {
        RewardCandidateMarket {
            market: RewardMarket {
                condition_id: format!("condition-{index}"),
                question: format!("Question {index}"),
                market_slug: format!("market-{index}"),
                event_slug: "event".to_string(),
                category: String::new(),
                image: String::new(),
                rewards_max_spread: Decimal::ONE,
                rewards_min_size: Decimal::ONE,
                total_daily_rate: Decimal::ONE,
                liquidity_usd: Decimal::ZERO,
                volume_24h_usd: Decimal::ZERO,
                market_spread_cents: Decimal::ZERO,
                end_at: None,
                ambiguity_level: String::new(),
                market_synced_at: None,
                tokens: vec![
                    RewardToken {
                        token_id: format!("yes-{index}"),
                        outcome: "Yes".to_string(),
                        price: None,
                    },
                    RewardToken {
                        token_id: format!("no-{index}"),
                        outcome: "No".to_string(),
                        price: None,
                    },
                ],
                active: true,
                updated_at: OffsetDateTime::now_utc(),
            },
            strategy_bucket: RewardStrategyBucket::LowCompetition,
        }
    }

    #[test]
    fn low_competition_probe_selects_ten_markets_and_rotates_after_current_tail() {
        let candidates = (0..12).map(probe_candidate).collect::<Vec<_>>();
        let low_candidates = low_competition_probe_candidates(&candidates);

        let first = select_low_competition_probe_conditions(&low_candidates, &[]);
        assert_eq!(first.len(), REWARD_LOW_COMPETITION_PROBE_MARKETS_PER_BATCH);
        assert_eq!(first.first().map(String::as_str), Some("condition-0"));
        assert_eq!(first.last().map(String::as_str), Some("condition-9"));

        let second = select_low_competition_probe_conditions(&low_candidates, &first);
        assert_eq!(second.first().map(String::as_str), Some("condition-10"));
        assert_eq!(second.get(1).map(String::as_str), Some("condition-11"));
        assert_eq!(second.get(2).map(String::as_str), Some("condition-0"));
    }

    #[test]
    fn low_competition_probe_token_ids_follow_selected_conditions() {
        let candidates = (0..3).map(probe_candidate).collect::<Vec<_>>();
        let low_candidates = low_competition_probe_candidates(&candidates);
        let token_ids = low_competition_probe_token_ids(
            &low_candidates,
            &["condition-1".to_string(), "condition-2".to_string()],
        );

        assert_eq!(
            token_ids,
            vec![
                "yes-1".to_string(),
                "no-1".to_string(),
                "yes-2".to_string(),
                "no-2".to_string(),
            ]
        );
    }
}
