impl InMemoryMarketEventStore {
async fn market_event_recompute_signal(
        &self,
        command: &RecomputeSignalCommand,
    ) -> Result<RecomputeSignalResult> {
        let signal = {
            let signals = self.signals.read().await;
            signals.get(&command.signal_id).cloned().ok_or_else(|| {
                AppError::not_found(
                    "SIGNAL_NOT_FOUND",
                    format!("signal was not found: {}", command.signal_id),
                )
            })?
        };
        let market = {
            let markets = self.markets.read().await;
            markets.get(&signal.market_id).cloned().ok_or_else(|| {
                AppError::not_found(
                    "MARKET_NOT_FOUND",
                    format!("market was not found: {}", signal.market_id),
                )
            })?
        };
        let evidences: Vec<_> = {
            let evidences = self.evidences.read().await;
            evidences
                .values()
                .filter(|evidence| {
                    evidence.market_id == signal.market_id && evidence.event_id == signal.event_id
                })
                .cloned()
                .collect()
        };
        let source_health = self
            .source_health_adjustment_for_event(&signal.event_id)
            .await;

        let estimate_id = format!("est_{}", Uuid::now_v7());
        let draft = build_recompute_signal_draft_with_source_health(
            &signal,
            &market,
            &evidences,
            &command.reason,
            source_health.as_ref(),
            &estimate_id,
        )?;

        {
            let mut estimates = self.probability_estimates.write().await;
            estimates.insert(draft.estimate.id.clone(), draft.estimate.clone());
        }

        {
            let mut signals = self.signals.write().await;
            signals.insert(draft.next_signal.id.clone(), draft.next_signal.clone());
        }

        let transition = if let Some(transition) = draft.transition {
            let view = SignalTransitionView {
                id: format!("sgt_{}", Uuid::now_v7()),
                signal_id: draft.next_signal.id.clone(),
                from_state: transition.from_state,
                to_state: transition.to_state,
                trigger_type: transition.trigger_type,
                trigger_payload: transition.trigger_payload,
                created_at: transition.created_at,
            };
            self.signal_transitions.write().await.push(view.clone());
            Some(view)
        } else {
            None
        };

        Ok(RecomputeSignalResult {
            signal: draft.next_signal,
            estimate: draft.estimate,
            transition,
        })
    }

async fn market_event_approve_signal(
        &self,
        signal_id: &str,
        approved_by_user_id: &str,
        approval_reason: &str,
        _trace_id: &str,
        expected_version: Option<i64>,
    ) -> Result<SignalView> {
        let mut signals = self.signals.write().await;
        let current = signals.get(signal_id).cloned().ok_or_else(|| {
            AppError::not_found(
                "SIGNAL_NOT_FOUND",
                format!("signal was not found: {signal_id}"),
            )
        })?;

        if let Some(expected_version) = expected_version {
            if current.version != expected_version {
                return Err(AppError::conflict(
                    "STATE_VERSION_MISMATCH",
                    "signal version does not match the expected_version",
                ));
            }
        }

        if current.approved_by_user_id.is_some() {
            return Err(AppError::conflict(
                "STATE_SIGNAL_ALREADY_APPROVED",
                "signal has already been approved",
            ));
        }

        if current.rejected_by_user_id.is_some() {
            return Err(AppError::conflict(
                "STATE_SIGNAL_ALREADY_REJECTED",
                "signal has already been rejected for the current version",
            ));
        }

        let approved_at = OffsetDateTime::now_utc();
        let approved_signal = SignalView {
            id: current.id.clone(),
            market_id: current.market_id.clone(),
            event_id: current.event_id.clone(),
            action: current.action,
            side: current.side,
            market_price: current.market_price,
            fair_price: current.fair_price,
            edge: current.edge,
            confidence: current.confidence,
            lifecycle_state: current.lifecycle_state,
            reason: current.reason.clone(),
            risk_decision: approval_reason.to_string(),
            evidence_ids: current.evidence_ids.clone(),
            approved_by_user_id: Some(approved_by_user_id.to_string()),
            approved_at: Some(approved_at),
            rejected_by_user_id: None,
            rejected_at: None,
            updated_at: approved_at,
            version: current.version + 1,
        };

        signals.insert(signal_id.to_string(), approved_signal.clone());
        Ok(approved_signal)
    }

async fn market_event_reject_signal(
        &self,
        signal_id: &str,
        rejected_by_user_id: &str,
        rejection_reason: &str,
        _trace_id: &str,
        expected_version: Option<i64>,
    ) -> Result<SignalView> {
        let mut signals = self.signals.write().await;
        let current = signals.get(signal_id).cloned().ok_or_else(|| {
            AppError::not_found(
                "SIGNAL_NOT_FOUND",
                format!("signal was not found: {signal_id}"),
            )
        })?;

        if let Some(expected_version) = expected_version {
            if current.version != expected_version {
                return Err(AppError::conflict(
                    "STATE_VERSION_MISMATCH",
                    "signal version does not match the expected_version",
                ));
            }
        }

        if current.approved_by_user_id.is_some() {
            return Err(AppError::conflict(
                "STATE_SIGNAL_ALREADY_APPROVED",
                "approved signals cannot be rejected for the current version",
            ));
        }

        if current.rejected_by_user_id.is_some() {
            return Err(AppError::conflict(
                "STATE_SIGNAL_ALREADY_REJECTED",
                "signal has already been rejected for the current version",
            ));
        }

        let rejected_at = OffsetDateTime::now_utc();
        let rejected_signal = SignalView {
            id: current.id.clone(),
            market_id: current.market_id.clone(),
            event_id: current.event_id.clone(),
            action: current.action,
            side: current.side,
            market_price: current.market_price,
            fair_price: current.fair_price,
            edge: current.edge,
            confidence: current.confidence,
            lifecycle_state: current.lifecycle_state,
            reason: current.reason.clone(),
            risk_decision: rejection_reason.to_string(),
            evidence_ids: current.evidence_ids.clone(),
            approved_by_user_id: None,
            approved_at: None,
            rejected_by_user_id: Some(rejected_by_user_id.to_string()),
            rejected_at: Some(rejected_at),
            updated_at: rejected_at,
            version: current.version + 1,
        };

        signals.insert(signal_id.to_string(), rejected_signal.clone());
        Ok(rejected_signal)
    }
}
