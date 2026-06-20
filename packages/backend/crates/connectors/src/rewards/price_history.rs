impl PolymarketRewardsConnector {
    pub async fn fetch_price_history(
        &self,
        token_id: &str,
        start: OffsetDateTime,
        end: OffsetDateTime,
        fidelity_minutes: u16,
    ) -> Result<Vec<PolymarketPriceHistoryPoint>> {
        let token_id = token_id.trim();
        if token_id.is_empty() {
            return Ok(Vec::new());
        }
        let mut url = reqwest::Url::parse(&format!("{}/prices-history", self.clob_host))
            .map_err(|error| {
                AppError::invalid_input(
                    "POLYMARKET_PRICE_HISTORY_URL_INVALID",
                    format!("failed to construct Polymarket price history URL: {error}"),
                )
            })?;
        url.query_pairs_mut()
            .append_pair("market", token_id)
            .append_pair("startTs", &start.unix_timestamp().to_string())
            .append_pair("endTs", &end.unix_timestamp().to_string())
            .append_pair("fidelity", &fidelity_minutes.max(1).to_string());

        let response = self.client.get(url).send().await.map_err(|error| {
            AppError::dependency_unavailable(
                "POLYMARKET_PRICE_HISTORY_REQUEST_FAILED",
                format!("failed to request Polymarket price history for token_id={token_id}: {error}"),
            )
        })?;
        let status = response.status();
        if status == reqwest::StatusCode::NOT_FOUND {
            return Ok(Vec::new());
        }
        if !status.is_success() {
            return Err(AppError::dependency_unavailable(
                "POLYMARKET_PRICE_HISTORY_STATUS_FAILED",
                format!("Polymarket price history returned HTTP {status} for token_id={token_id}"),
            ));
        }

        let payload = response
            .json::<RawPriceHistoryResponse>()
            .await
            .map_err(|error| {
                AppError::dependency_unavailable(
                    "POLYMARKET_PRICE_HISTORY_DECODE_FAILED",
                    format!(
                        "failed to decode Polymarket price history for token_id={token_id}: {error}"
                    ),
                )
            })?;
        Ok(map_price_history_points(payload.history.unwrap_or_default()))
    }
}

fn map_price_history_points(raws: Vec<RawPriceHistoryPoint>) -> Vec<PolymarketPriceHistoryPoint> {
    let mut points = raws
        .into_iter()
        .filter_map(map_price_history_point)
        .collect::<Vec<_>>();
    points.sort_by_key(|point| point.observed_at);
    points.dedup_by_key(|point| point.observed_at);
    points
}

fn map_price_history_point(raw: RawPriceHistoryPoint) -> Option<PolymarketPriceHistoryPoint> {
    let observed_at = parse_price_history_timestamp(raw.t?)?;
    let price = parse_price_history_price(raw.p?)?;
    Some(PolymarketPriceHistoryPoint { observed_at, price })
}

fn parse_price_history_timestamp(value: i64) -> Option<OffsetDateTime> {
    // CLOB history has historically used unix seconds, but accept millis to
    // tolerate provider shape changes without creating implausible future rows.
    let seconds = if value.abs() > 10_000_000_000 {
        value / 1_000
    } else {
        value
    };
    OffsetDateTime::from_unix_timestamp(seconds).ok()
}

fn parse_price_history_price(value: serde_json::Value) -> Option<Decimal> {
    let price = match value {
        serde_json::Value::Number(number) => Decimal::from_str(&number.to_string()).ok()?,
        serde_json::Value::String(value) => Decimal::from_str(value.trim()).ok()?,
        _ => return None,
    };
    (price >= Decimal::ZERO && price <= Decimal::ONE).then_some(price.round_dp(8))
}

#[cfg(test)]
mod price_history_tests {
    use super::*;

    #[test]
    fn price_history_points_accept_seconds_and_millis() {
        let points = map_price_history_points(vec![
            RawPriceHistoryPoint {
                t: Some(1_700_000_000_000),
                p: Some(serde_json::json!("0.42")),
            },
            RawPriceHistoryPoint {
                t: Some(1_700_000_300),
                p: Some(serde_json::json!(0.43)),
            },
            RawPriceHistoryPoint {
                t: Some(1_700_000_300),
                p: Some(serde_json::json!(0.44)),
            },
        ]);

        assert_eq!(points.len(), 2);
        assert_eq!(
            points[0].observed_at.unix_timestamp(),
            1_700_000_000
        );
        assert_eq!(points[0].price, Decimal::from_str_exact("0.42").unwrap());
        assert_eq!(points[1].price, Decimal::from_str_exact("0.43").unwrap());
    }

    #[test]
    fn price_history_points_skip_out_of_range_prices() {
        let points = map_price_history_points(vec![
            RawPriceHistoryPoint {
                t: Some(1_700_000_000),
                p: Some(serde_json::json!("1.25")),
            },
            RawPriceHistoryPoint {
                t: Some(1_700_000_300),
                p: Some(serde_json::json!("-0.01")),
            },
        ]);

        assert!(points.is_empty());
    }
}
