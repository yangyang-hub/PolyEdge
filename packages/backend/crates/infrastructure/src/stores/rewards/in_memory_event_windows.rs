impl InMemoryRewardBotStore {
    async fn replace_market_event_windows_inner(
        &self,
        snapshot: &RewardEventWindowSourceSnapshot,
    ) -> Result<RewardEventWindowReplaceReport> {
        let validated = validate_reward_event_window_snapshot(snapshot)?;
        let parent_condition_ids = self
            .markets
            .read()
            .await
            .keys()
            .cloned()
            .collect::<HashSet<_>>();
        let mut report = RewardEventWindowReplaceReport {
            source: snapshot.source.clone(),
            covered_condition_count: validated.covered_condition_ids.len(),
            input_window_count: snapshot.windows.len(),
            ..RewardEventWindowReplaceReport::default()
        };
        let mut source_versions = self.event_window_source_versions.write().await;
        let mut store = self.event_windows.write().await;

        let mut accepted_conditions = HashSet::new();
        for condition_id in &validated.covered_condition_ids {
            if !parent_condition_ids.contains(condition_id) {
                continue;
            }
            let key = (snapshot.source.clone(), condition_id.clone());
            let candidate_hash = validated
                .condition_hashes
                .get(condition_id)
                .expect("validated event-window condition hash");
            let candidate_source_updated_at = validated
                .condition_source_updated_at
                .get(condition_id)
                .copied()
                .flatten();
            match source_versions.get(&key) {
                Some((producer_version, source_updated_at, observed_at, _))
                    if reward_event_window_source_version_cmp(
                        snapshot.producer_version,
                        candidate_source_updated_at,
                        snapshot.observed_at,
                        *producer_version,
                        *source_updated_at,
                        *observed_at,
                    ) == std::cmp::Ordering::Less =>
                {
                    report.ignored_stale_condition_count += 1;
                    report.ignored_stale_count += u64::try_from(
                        snapshot
                            .windows
                            .iter()
                            .filter(|window| window.condition_id == *condition_id)
                            .count(),
                    )
                    .unwrap_or(u64::MAX);
                }
                Some((producer_version, source_updated_at, observed_at, existing_hash))
                    if reward_event_window_source_version_cmp(
                        snapshot.producer_version,
                        candidate_source_updated_at,
                        snapshot.observed_at,
                        *producer_version,
                        *source_updated_at,
                        *observed_at,
                    ) == std::cmp::Ordering::Equal =>
                {
                    if existing_hash != candidate_hash {
                        return Err(AppError::invalid_input(
                            "REWARD_EVENT_WINDOW_SNAPSHOT_CONFLICT",
                            format!(
                                "conflicting event-window snapshots share source={}, condition_id={}, producer_version={}, observed_at={}",
                                snapshot.source,
                                condition_id,
                                snapshot.producer_version,
                                snapshot.observed_at
                            ),
                        ));
                    }
                    report.idempotent_window_count += u64::try_from(
                        snapshot
                            .windows
                            .iter()
                            .filter(|window| window.condition_id == *condition_id)
                            .count(),
                    )
                    .unwrap_or(u64::MAX);
                }
                _ => {
                    accepted_conditions.insert(condition_id.clone());
                }
            }
        }

        for window in &snapshot.windows {
            if !parent_condition_ids.contains(&window.condition_id) {
                report.skipped_missing_parent_count += 1;
                continue;
            }
            if !accepted_conditions.contains(&window.condition_id) {
                continue;
            }
            let key = (
                window.condition_id.clone(),
                window.source.clone(),
                window.event_key.clone(),
            );
            let observed_at = window.observed_at.unwrap_or(snapshot.observed_at);
            let mut normalized = window.clone();
            normalized.observed_at = Some(observed_at);
            if let Some(existing) = store.get(&key) {
                match reward_event_window_version_cmp(&normalized, observed_at, existing) {
                    std::cmp::Ordering::Less => {
                        report.ignored_stale_count += 1;
                        continue;
                    }
                    std::cmp::Ordering::Equal if existing == &normalized => {
                        report.idempotent_window_count += 1;
                        continue;
                    }
                    std::cmp::Ordering::Equal => {
                        return Err(AppError::invalid_input(
                            "REWARD_EVENT_WINDOW_IDENTITY_CONFLICT",
                            format!(
                                "conflicting event-window payload shares condition_id={}, source={}, event_key={} and version fence",
                                window.condition_id, window.source, window.event_key
                            ),
                        ));
                    }
                    std::cmp::Ordering::Greater => {}
                }
            }
            store.insert(key, normalized);
            report.upserted_window_count += 1;
        }

        let covered = validated
            .covered_condition_ids
            .iter()
            .map(String::as_str)
            .collect::<HashSet<_>>();
        for ((condition_id, source, event_key), window) in store.iter_mut() {
            let coverage_source_updated_at = validated
                .condition_source_updated_at
                .get(condition_id)
                .copied()
                .flatten();
            if source != &snapshot.source
                || !covered.contains(condition_id.as_str())
                || !accepted_conditions.contains(condition_id)
                || validated
                    .incoming_identities
                    .contains(&(condition_id.clone(), event_key.clone()))
                || !window.active
                || reward_event_window_source_version_cmp(
                    snapshot.producer_version,
                    coverage_source_updated_at,
                    snapshot.observed_at,
                    window.producer_version,
                    window.source_updated_at,
                    window.observed_at.unwrap_or(window.updated_at),
                ) == std::cmp::Ordering::Less
            {
                continue;
            }
            window.active = false;
            window.hard_gate_eligible = false;
            window.schedule_status = RewardEventScheduleStatus::Withdrawn;
            window.producer_version = snapshot.producer_version;
            window.observed_at = Some(snapshot.observed_at);
            window.updated_at = snapshot.observed_at;
            report.deactivated_window_count += 1;
        }

        for condition_id in accepted_conditions {
            let hash = validated
                .condition_hashes
                .get(&condition_id)
                .expect("validated event-window condition hash")
                .clone();
            let source_updated_at = validated
                .condition_source_updated_at
                .get(&condition_id)
                .copied()
                .flatten();
            source_versions.insert(
                (snapshot.source.clone(), condition_id),
                (
                    snapshot.producer_version,
                    source_updated_at,
                    snapshot.observed_at,
                    hash,
                ),
            );
        }

        report.skipped_window_count = report
            .ignored_stale_count
            .saturating_add(report.idempotent_window_count)
            .saturating_add(report.skipped_missing_parent_count);
        Ok(report)
    }
}
