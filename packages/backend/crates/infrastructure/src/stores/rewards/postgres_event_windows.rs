async fn postgres_replace_reward_market_event_windows(
    pool: &PgPool,
    snapshot: &RewardEventWindowSourceSnapshot,
) -> Result<RewardEventWindowReplaceReport> {
    let validated = validate_reward_event_window_snapshot(snapshot)?;
    let mut report = RewardEventWindowReplaceReport {
        source: snapshot.source.clone(),
        covered_condition_count: validated.covered_condition_ids.len(),
        input_window_count: snapshot.windows.len(),
        ..RewardEventWindowReplaceReport::default()
    };

    let mut transaction = pool.begin().await.map_err(|error| {
        db_error(
            "POSTGRES_TRANSACTION_BEGIN_FAILED",
            format!("failed to begin reward event-window replacement: {error}"),
        )
    })?;
    sqlx::query(
        "SELECT pg_advisory_xact_lock(hashtextextended('reward_event_window:' || $1, 0))",
    )
    .bind(&snapshot.source)
    .execute(&mut *transaction)
    .await
    .map_err(|error| {
        db_error(
            "POSTGRES_ADVISORY_LOCK_FAILED",
            format!("failed to lock reward event-window source snapshot: {error}"),
        )
    })?;

    let parent_rows = if validated.covered_condition_ids.is_empty() {
        Vec::new()
    } else {
        sqlx::query(
            r#"
            SELECT condition_id
            FROM reward_markets
            WHERE condition_id = ANY($1)
            "#,
        )
        .bind(&validated.covered_condition_ids)
        .fetch_all(&mut *transaction)
        .await
        .map_err(|error| {
            db_error(
                "POSTGRES_QUERY_FAILED",
                format!("failed to resolve reward event-window parents: {error}"),
            )
        })?
    };
    let parent_condition_ids = parent_rows
        .into_iter()
        .map(|row| row.try_get::<String, _>("condition_id"))
        .collect::<std::result::Result<HashSet<_>, _>>()
        .map_err(postgres_decode_error)?;
    let source_version_rows = if parent_condition_ids.is_empty() {
        Vec::new()
    } else {
        let condition_ids = parent_condition_ids.iter().cloned().collect::<Vec<_>>();
        sqlx::query(
            r#"
            SELECT condition_id, producer_version, source_updated_at, observed_at, snapshot_hash
            FROM reward_event_window_source_versions
            WHERE source = $1
              AND condition_id = ANY($2)
            FOR UPDATE
            "#,
        )
        .bind(&snapshot.source)
        .bind(&condition_ids)
        .fetch_all(&mut *transaction)
        .await
        .map_err(|error| {
            db_error(
                "POSTGRES_QUERY_FAILED",
                format!("failed to load reward event-window source versions: {error}"),
            )
        })?
    };
    let mut source_versions = HashMap::new();
    for row in source_version_rows {
        let condition_id: String = row
            .try_get("condition_id")
            .map_err(postgres_decode_error)?;
        let producer_version: i64 = row
            .try_get("producer_version")
            .map_err(postgres_decode_error)?;
        let producer_version = u32::try_from(producer_version).map_err(|_| {
            db_error(
                "POSTGRES_DECODE_FAILED",
                format!("invalid reward event-window source version: {producer_version}"),
            )
        })?;
        let observed_at: OffsetDateTime = row
            .try_get("observed_at")
            .map_err(postgres_decode_error)?;
        let source_updated_at: Option<OffsetDateTime> = row
            .try_get("source_updated_at")
            .map_err(postgres_decode_error)?;
        let snapshot_hash: String = row
            .try_get("snapshot_hash")
            .map_err(postgres_decode_error)?;
        source_versions.insert(
            condition_id,
            (
                producer_version,
                source_updated_at,
                observed_at,
                snapshot_hash,
            ),
        );
    }

    let mut accepted_condition_ids = Vec::new();
    for condition_id in &validated.covered_condition_ids {
        if !parent_condition_ids.contains(condition_id) {
            continue;
        }
        let candidate_hash = validated
            .condition_hashes
            .get(condition_id)
            .expect("validated event-window condition hash");
        let candidate_source_updated_at = validated
            .condition_source_updated_at
            .get(condition_id)
            .copied()
            .flatten();
        match source_versions.get(condition_id) {
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
            _ => accepted_condition_ids.push(condition_id.clone()),
        }
    }
    let accepted_conditions = accepted_condition_ids
        .iter()
        .map(String::as_str)
        .collect::<HashSet<_>>();
    let mut persisted_windows = snapshot
        .windows
        .iter()
        .filter(|window| accepted_conditions.contains(window.condition_id.as_str()))
        .collect::<Vec<_>>();
    persisted_windows.sort_by(|left, right| {
        left.condition_id
            .cmp(&right.condition_id)
            .then_with(|| left.event_key.cmp(&right.event_key))
    });
    report.skipped_missing_parent_count = u64::try_from(
        snapshot
            .windows
            .iter()
            .filter(|window| !parent_condition_ids.contains(&window.condition_id))
            .count(),
    )
    .unwrap_or(u64::MAX);

    for chunk in persisted_windows.chunks(REWARD_UPSERT_BATCH_SIZE) {
        let mut builder: QueryBuilder<Postgres> = QueryBuilder::new(
            r#"
            INSERT INTO reward_market_event_windows (
              condition_id,
              source,
              event_key,
              event_type,
              event_time_role,
              schedule_status,
              time_precision,
              start_source_field,
              end_policy,
              event_start_at,
              event_end_at,
              confidence,
              source_url,
              source_payload,
              notes,
              active,
              hard_gate_eligible,
              producer_version,
              source_updated_at,
              observed_at,
              expires_at,
              reviewed_by,
              reviewed_at,
              updated_at
            )
            "#,
        );
        builder.push_values(chunk.iter(), |mut row, window| {
            let observed_at = window.observed_at.unwrap_or(snapshot.observed_at);
            row.push_bind(&window.condition_id)
                .push_bind(&window.source)
                .push_bind(&window.event_key)
                .push_bind(&window.event_type)
                .push_bind(window.event_time_role.as_str())
                .push_bind(window.schedule_status.as_str())
                .push_bind(window.time_precision.as_str())
                .push_bind(&window.start_source_field)
                .push_bind(window.end_policy.as_str())
                .push_bind(window.event_start_at)
                .push_bind(window.event_end_at)
                .push_bind(window.confidence.as_str())
                .push_bind(&window.source_url)
                .push_bind(Json(window.source_payload.clone()))
                .push_bind(&window.notes)
                .push_bind(window.active)
                .push_bind(window.hard_gate_eligible)
                .push_bind(i64::from(window.producer_version))
                .push_bind(window.source_updated_at)
                .push_bind(observed_at)
                .push_bind(window.expires_at)
                .push_bind(&window.reviewed_by)
                .push_bind(window.reviewed_at)
                .push_bind(window.updated_at);
        });
        builder.push(
            r#"
            ON CONFLICT (condition_id, source, event_key) DO UPDATE
            SET event_type = EXCLUDED.event_type,
                event_time_role = EXCLUDED.event_time_role,
                schedule_status = EXCLUDED.schedule_status,
                time_precision = EXCLUDED.time_precision,
                start_source_field = EXCLUDED.start_source_field,
                end_policy = EXCLUDED.end_policy,
                event_start_at = EXCLUDED.event_start_at,
                event_end_at = EXCLUDED.event_end_at,
                confidence = EXCLUDED.confidence,
                source_url = EXCLUDED.source_url,
                source_payload = EXCLUDED.source_payload,
                notes = EXCLUDED.notes,
                active = EXCLUDED.active,
                hard_gate_eligible = EXCLUDED.hard_gate_eligible,
                producer_version = EXCLUDED.producer_version,
                source_updated_at = EXCLUDED.source_updated_at,
                observed_at = EXCLUDED.observed_at,
                expires_at = EXCLUDED.expires_at,
                reviewed_by = EXCLUDED.reviewed_by,
                reviewed_at = EXCLUDED.reviewed_at,
                updated_at = EXCLUDED.updated_at
            WHERE (
                EXCLUDED.producer_version,
                COALESCE(EXCLUDED.source_updated_at, '-infinity'::TIMESTAMPTZ),
                COALESCE(EXCLUDED.observed_at, EXCLUDED.updated_at)
            ) > (
                reward_market_event_windows.producer_version,
                COALESCE(
                    reward_market_event_windows.source_updated_at,
                    '-infinity'::TIMESTAMPTZ
                ),
                COALESCE(
                    reward_market_event_windows.observed_at,
                    reward_market_event_windows.updated_at
                )
            )
            "#,
        );
        let affected = builder
            .build()
            .execute(&mut *transaction)
            .await
            .map_err(|error| {
                db_error(
                    "POSTGRES_UPSERT_FAILED",
                    format!(
                        "failed to batch upsert reward event windows (chunk size {}): {error}",
                        chunk.len()
                    ),
                )
            })?
            .rows_affected();
        report.upserted_window_count = report.upserted_window_count.saturating_add(affected);
        report.ignored_stale_count = report.ignored_stale_count.saturating_add(
            u64::try_from(chunk.len())
                .unwrap_or(u64::MAX)
                .saturating_sub(affected),
        );
    }

    if !accepted_condition_ids.is_empty() {
        let incoming_condition_ids = persisted_windows
            .iter()
            .map(|window| window.condition_id.clone())
            .collect::<Vec<_>>();
        let incoming_event_keys = persisted_windows
            .iter()
            .map(|window| window.event_key.clone())
            .collect::<Vec<_>>();
        let coverage_source_updated_at = accepted_condition_ids
            .iter()
            .map(|condition_id| {
                validated
                    .condition_source_updated_at
                    .get(condition_id)
                    .copied()
                    .flatten()
            })
            .collect::<Vec<_>>();
        report.deactivated_window_count = sqlx::query(
            r#"
            UPDATE reward_market_event_windows AS existing
            SET active = FALSE,
                hard_gate_eligible = FALSE,
                schedule_status = 'withdrawn',
                producer_version = $4,
                source_updated_at = coverage.source_updated_at,
                observed_at = $3,
                updated_at = $3
            FROM unnest($2::TEXT[], $5::TIMESTAMPTZ[])
                AS coverage(condition_id, source_updated_at)
            WHERE existing.source = $1
              AND existing.condition_id = coverage.condition_id
              AND existing.active = TRUE
              AND (
                  existing.producer_version,
                  COALESCE(existing.source_updated_at, '-infinity'::TIMESTAMPTZ),
                  COALESCE(existing.observed_at, existing.updated_at)
              ) <= (
                  $4,
                  COALESCE(coverage.source_updated_at, '-infinity'::TIMESTAMPTZ),
                  $3
              )
              AND NOT EXISTS (
                  SELECT 1
                  FROM unnest($6::TEXT[], $7::TEXT[])
                      AS incoming(condition_id, event_key)
                  WHERE incoming.condition_id = existing.condition_id
                    AND incoming.event_key = existing.event_key
              )
            "#,
        )
        .bind(&snapshot.source)
        .bind(&accepted_condition_ids)
        .bind(snapshot.observed_at)
        .bind(i64::from(snapshot.producer_version))
        .bind(&coverage_source_updated_at)
        .bind(&incoming_condition_ids)
        .bind(&incoming_event_keys)
        .execute(&mut *transaction)
        .await
        .map_err(|error| {
            db_error(
                "POSTGRES_UPDATE_FAILED",
                format!("failed to tombstone missing reward event-window keys: {error}"),
            )
        })?
        .rows_affected();

        let mut builder: QueryBuilder<Postgres> = QueryBuilder::new(
            r#"
            INSERT INTO reward_event_window_source_versions (
              source,
              condition_id,
              producer_version,
              source_updated_at,
              observed_at,
              snapshot_hash,
              updated_at
            )
            "#,
        );
        builder.push_values(accepted_condition_ids.iter(), |mut row, condition_id| {
            row.push_bind(&snapshot.source)
                .push_bind(condition_id)
                .push_bind(i64::from(snapshot.producer_version))
                .push_bind(
                    validated
                        .condition_source_updated_at
                        .get(condition_id)
                        .copied()
                        .flatten(),
                )
                .push_bind(snapshot.observed_at)
                .push_bind(
                    validated
                        .condition_hashes
                        .get(condition_id)
                        .expect("validated event-window condition hash"),
                )
                .push_bind(snapshot.observed_at);
        });
        builder.push(
            r#"
            ON CONFLICT (source, condition_id) DO UPDATE
            SET producer_version = EXCLUDED.producer_version,
                source_updated_at = EXCLUDED.source_updated_at,
                observed_at = EXCLUDED.observed_at,
                snapshot_hash = EXCLUDED.snapshot_hash,
                updated_at = EXCLUDED.updated_at
            WHERE (
                EXCLUDED.producer_version,
                COALESCE(EXCLUDED.source_updated_at, '-infinity'::TIMESTAMPTZ),
                EXCLUDED.observed_at
            ) > (
                reward_event_window_source_versions.producer_version,
                COALESCE(
                    reward_event_window_source_versions.source_updated_at,
                    '-infinity'::TIMESTAMPTZ
                ),
                reward_event_window_source_versions.observed_at
            )
            "#,
        );
        builder
            .build()
            .execute(&mut *transaction)
            .await
            .map_err(|error| {
                db_error(
                    "POSTGRES_UPSERT_FAILED",
                    format!("failed to update reward event-window source versions: {error}"),
                )
            })?;
    }

    transaction.commit().await.map_err(|error| {
        db_error(
            "POSTGRES_TRANSACTION_COMMIT_FAILED",
            format!("failed to commit reward event-window replacement: {error}"),
        )
    })?;
    report.skipped_window_count = report
        .ignored_stale_count
        .saturating_add(report.idempotent_window_count)
        .saturating_add(report.skipped_missing_parent_count);
    Ok(report)
}

async fn postgres_list_reward_market_event_windows(
    pool: &PgPool,
    condition_ids: &[String],
    as_of: OffsetDateTime,
) -> Result<Vec<RewardMarketEventWindow>> {
    if condition_ids.is_empty() {
        return Ok(Vec::new());
    }

    let rows = sqlx::query(
        r#"
        SELECT condition_id,
               source,
               event_key,
               event_type,
               event_time_role,
               schedule_status,
               time_precision,
               start_source_field,
               end_policy,
               event_start_at,
               event_end_at,
               confidence,
               source_url,
               source_payload,
               notes,
               active,
               hard_gate_eligible,
               producer_version,
               source_updated_at,
               observed_at,
               expires_at,
               reviewed_by,
               reviewed_at,
               updated_at
        FROM reward_market_event_windows
        WHERE active = TRUE
          AND condition_id = ANY($1)
          AND (expires_at IS NULL OR expires_at > $2)
        ORDER BY condition_id ASC, source ASC, event_key ASC
        "#,
    )
    .bind(condition_ids)
    .bind(as_of)
    .fetch_all(pool)
    .await
    .map_err(|error| {
        db_error(
            "POSTGRES_QUERY_FAILED",
            format!("failed to query reward market event windows: {error}"),
        )
    })?;

    rows.into_iter()
        .map(reward_market_event_window_from_row)
        .collect()
}

fn reward_market_event_window_from_row(
    row: sqlx::postgres::PgRow,
) -> Result<RewardMarketEventWindow> {
    let event_time_role: String = row
        .try_get("event_time_role")
        .map_err(postgres_decode_error)?;
    let schedule_status: String = row
        .try_get("schedule_status")
        .map_err(postgres_decode_error)?;
    let time_precision: String = row
        .try_get("time_precision")
        .map_err(postgres_decode_error)?;
    let end_policy: String = row.try_get("end_policy").map_err(postgres_decode_error)?;
    let confidence: String = row.try_get("confidence").map_err(postgres_decode_error)?;
    let producer_version: i64 = row
        .try_get("producer_version")
        .map_err(postgres_decode_error)?;
    let source_payload: Json<Value> = row
        .try_get("source_payload")
        .map_err(postgres_decode_error)?;

    Ok(RewardMarketEventWindow {
        condition_id: row.try_get("condition_id").map_err(postgres_decode_error)?,
        source: row.try_get("source").map_err(postgres_decode_error)?,
        event_key: row.try_get("event_key").map_err(postgres_decode_error)?,
        event_type: row.try_get("event_type").map_err(postgres_decode_error)?,
        event_time_role: RewardEventTimeRole::from_str(&event_time_role)?,
        schedule_status: RewardEventScheduleStatus::from_str(&schedule_status)?,
        time_precision: RewardEventTimePrecision::from_str(&time_precision)?,
        start_source_field: row
            .try_get("start_source_field")
            .map_err(postgres_decode_error)?,
        end_policy: RewardEventEndPolicy::from_str(&end_policy)?,
        event_start_at: row
            .try_get("event_start_at")
            .map_err(postgres_decode_error)?,
        event_end_at: row
            .try_get("event_end_at")
            .map_err(postgres_decode_error)?,
        confidence: RewardEventTimeConfidence::from_str(&confidence)?,
        source_url: row.try_get("source_url").map_err(postgres_decode_error)?,
        source_payload: source_payload.0,
        notes: row.try_get("notes").map_err(postgres_decode_error)?,
        active: row.try_get("active").map_err(postgres_decode_error)?,
        hard_gate_eligible: row
            .try_get("hard_gate_eligible")
            .map_err(postgres_decode_error)?,
        producer_version: u32::try_from(producer_version).map_err(|_| {
            AppError::dependency_unavailable(
                "POSTGRES_DECODE_FAILED",
                format!("invalid reward event-window producer version: {producer_version}"),
            )
        })?,
        source_updated_at: row
            .try_get("source_updated_at")
            .map_err(postgres_decode_error)?,
        observed_at: row.try_get("observed_at").map_err(postgres_decode_error)?,
        expires_at: row.try_get("expires_at").map_err(postgres_decode_error)?,
        reviewed_by: row.try_get("reviewed_by").map_err(postgres_decode_error)?,
        reviewed_at: row.try_get("reviewed_at").map_err(postgres_decode_error)?,
        updated_at: row.try_get("updated_at").map_err(postgres_decode_error)?,
    })
}
