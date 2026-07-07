#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
struct HighProbabilityOutcomeImportReport {
    rows_read: usize,
    outcomes_upserted: usize,
}

const HIGH_PROBABILITY_OBSERVE_ORDERBOOK_MAX_AGE_MS: i64 = 60_000;

#[derive(Debug, Deserialize)]
struct HighProbabilityOutcomeImportRow {
    condition_id: String,
    status: HighProbabilityMarketOutcomeStatus,
    winning_token_id: Option<String>,
    resolved_at: Option<String>,
    #[serde(default)]
    market_type: String,
    #[serde(default)]
    risk_tags: Vec<String>,
    #[serde(default)]
    label_source: String,
    #[serde(default = "default_high_probability_outcome_raw")]
    raw: Value,
}

async fn refresh_high_probability_buckets_once(
    state: &AppState,
) -> Result<HighProbabilityBucketRefreshReport> {
    state.high_probability_service.refresh_bucket_stats().await
}

async fn build_high_probability_samples_once(
    state: &AppState,
    limit: Option<u32>,
) -> Result<HighProbabilitySampleBuildReport> {
    state
        .high_probability_service
        .build_reward_candle_samples(limit)
        .await
}

async fn run_high_probability_backtest_once(
    state: &AppState,
) -> Result<HighProbabilityBacktestPersistReport> {
    state
        .high_probability_service
        .run_and_record_backtest()
        .await
}

async fn observe_high_probability_once(
    state: &AppState,
    limit: Option<u16>,
    trace_id: &str,
) -> Result<HighProbabilityObserveReport> {
    let candidates = state
        .high_probability_service
        .list_observe_candidates(limit)
        .await?;
    let token_ids = candidates
        .iter()
        .map(|candidate| candidate.token_id.clone())
        .filter(|token_id| !token_id.trim().is_empty())
        .collect::<HashSet<_>>()
        .into_iter()
        .collect::<Vec<_>>();
    let quotes = if token_ids.is_empty() {
        Vec::new()
    } else {
        match state
            .orderbook_cache
            .get_books_with_max_age(&token_ids, HIGH_PROBABILITY_OBSERVE_ORDERBOOK_MAX_AGE_MS)
            .await
        {
            Ok(books) => high_probability_quotes_from_books(&books),
            Err(error) => {
                warn!(
                    trace_id = %trace_id,
                    error = %error,
                    token_count = token_ids.len(),
                    "high probability observe skipped orderbook quotes because cache read failed"
                );
                Vec::new()
            }
        }
    };

    state
        .high_probability_service
        .observe_candidates(&candidates, &quotes)
        .await
}

async fn refresh_high_probability_fair_values_once(
    state: &AppState,
    limit: Option<u16>,
    trace_id: &str,
) -> Result<FairValueRefreshReport> {
    let candidates = state
        .high_probability_service
        .list_observe_candidates(limit)
        .await?;
    let token_ids = candidates
        .iter()
        .map(|candidate| candidate.token_id.clone())
        .filter(|token_id| !token_id.trim().is_empty())
        .collect::<HashSet<_>>()
        .into_iter()
        .collect::<Vec<_>>();
    let quotes = if token_ids.is_empty() {
        Vec::new()
    } else {
        match state
            .orderbook_cache
            .get_books_with_max_age(&token_ids, HIGH_PROBABILITY_OBSERVE_ORDERBOOK_MAX_AGE_MS)
            .await
        {
            Ok(books) => high_probability_quotes_from_books(&books),
            Err(error) => {
                warn!(
                    trace_id = %trace_id,
                    error = %error,
                    token_count = token_ids.len(),
                    "high probability fair value refresh skipped orderbook quotes because cache read failed"
                );
                Vec::new()
            }
        }
    };

    state
        .high_probability_service
        .refresh_fair_values(&candidates, &quotes)
        .await
}

fn high_probability_quotes_from_books(books: &[CachedOrderBook]) -> Vec<HighProbabilityOrderbookQuote> {
    books
        .iter()
        .map(|book| {
            let best_bid = book.bids.first().map(|level| level.price);
            let best_ask = book.asks.first().map(|level| level.price);
            let ask_depth_usd = book
                .asks
                .first()
                .map(|level| level.price * level.size);
            HighProbabilityOrderbookQuote {
                token_id: book.token_id.clone(),
                best_bid,
                best_ask,
                ask_depth_usd,
                confirmed_at_ms: Some(book.confirmation_time_ms()),
            }
        })
        .collect()
}

async fn import_high_probability_outcomes_once(
    state: &AppState,
    path: &str,
) -> Result<HighProbabilityOutcomeImportReport> {
    let contents = tokio::fs::read_to_string(path).await.map_err(|error| {
        AppError::invalid_input(
            "HIGH_PROBABILITY_OUTCOME_IMPORT_READ_FAILED",
            format!("failed to read high probability outcome import file {path}: {error}"),
        )
    })?;
    let rows = parse_high_probability_outcome_import_rows(&contents)?;
    let rows_read = rows.len();
    let mut outcomes_upserted = 0usize;

    for (index, row) in rows.into_iter().enumerate() {
        let outcome = high_probability_outcome_from_import_row(index + 1, row)?;
        state
            .high_probability_service
            .upsert_market_outcome(outcome)
            .await?;
        outcomes_upserted += 1;
    }

    Ok(HighProbabilityOutcomeImportReport {
        rows_read,
        outcomes_upserted,
    })
}

fn parse_high_probability_outcome_import_rows(
    contents: &str,
) -> Result<Vec<HighProbabilityOutcomeImportRow>> {
    let value = serde_json::from_str::<Value>(contents).map_err(|error| {
        AppError::invalid_input(
            "HIGH_PROBABILITY_OUTCOME_IMPORT_JSON_INVALID",
            format!("high probability outcome import file must be valid JSON: {error}"),
        )
    })?;
    let rows_value = match value {
        Value::Array(_) => value,
        Value::Object(mut object) => object.remove("outcomes").ok_or_else(|| {
            AppError::invalid_input(
                "HIGH_PROBABILITY_OUTCOME_IMPORT_OUTCOMES_MISSING",
                "high probability outcome import object must contain an outcomes array",
            )
        })?,
        _ => {
            return Err(AppError::invalid_input(
                "HIGH_PROBABILITY_OUTCOME_IMPORT_SHAPE_INVALID",
                "high probability outcome import file must be an array or an object with outcomes",
            ));
        }
    };

    serde_json::from_value::<Vec<HighProbabilityOutcomeImportRow>>(rows_value).map_err(|error| {
        AppError::invalid_input(
            "HIGH_PROBABILITY_OUTCOME_IMPORT_ROWS_INVALID",
            format!("high probability outcome import rows are invalid: {error}"),
        )
    })
}

fn high_probability_outcome_from_import_row(
    row_number: usize,
    row: HighProbabilityOutcomeImportRow,
) -> Result<HighProbabilityMarketOutcome> {
    let condition_id = row.condition_id.trim().to_string();
    if condition_id.is_empty() {
        return Err(AppError::invalid_input(
            "HIGH_PROBABILITY_OUTCOME_IMPORT_CONDITION_ID_MISSING",
            format!("row {row_number} is missing condition_id"),
        ));
    }

    let winning_token_id = row
        .winning_token_id
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty());
    let resolved_at =
        parse_high_probability_outcome_resolved_at(row_number, row.status, row.resolved_at)?;

    if matches!(row.status, HighProbabilityMarketOutcomeStatus::Resolved)
        && winning_token_id.is_none()
    {
        return Err(AppError::invalid_input(
            "HIGH_PROBABILITY_OUTCOME_IMPORT_WINNING_TOKEN_MISSING",
            format!("row {row_number} has status=resolved but no winning_token_id"),
        ));
    }

    Ok(HighProbabilityMarketOutcome {
        condition_id,
        status: row.status,
        winning_token_id,
        resolved_at,
        market_type: row.market_type,
        risk_tags: row.risk_tags,
        label_source: row.label_source,
        raw: row.raw,
        updated_at: OffsetDateTime::now_utc(),
    })
}

fn parse_high_probability_outcome_resolved_at(
    row_number: usize,
    status: HighProbabilityMarketOutcomeStatus,
    raw: Option<String>,
) -> Result<Option<OffsetDateTime>> {
    let Some(raw) = raw else {
        if matches!(status, HighProbabilityMarketOutcomeStatus::Resolved) {
            return Err(AppError::invalid_input(
                "HIGH_PROBABILITY_OUTCOME_IMPORT_RESOLVED_AT_MISSING",
                format!("row {row_number} has status=resolved but no resolved_at"),
            ));
        }
        return Ok(None);
    };
    let raw = raw.trim();
    if raw.is_empty() {
        if matches!(status, HighProbabilityMarketOutcomeStatus::Resolved) {
            return Err(AppError::invalid_input(
                "HIGH_PROBABILITY_OUTCOME_IMPORT_RESOLVED_AT_MISSING",
                format!("row {row_number} has status=resolved but no resolved_at"),
            ));
        }
        return Ok(None);
    }

    OffsetDateTime::parse(raw, &Rfc3339)
        .map(Some)
        .map_err(|error| {
            AppError::invalid_input(
                "HIGH_PROBABILITY_OUTCOME_IMPORT_RESOLVED_AT_INVALID",
                format!("row {row_number} resolved_at must be RFC3339: {error}"),
            )
        })
}

fn default_high_probability_outcome_raw() -> Value {
    json!({})
}
