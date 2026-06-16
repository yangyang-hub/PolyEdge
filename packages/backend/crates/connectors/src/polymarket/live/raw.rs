impl LivePolymarketConnector {
    async fn raw_post_heartbeat(&self, heartbeat_id: Option<&str>) -> Result<String> {
        let body = serde_json::json!({ "heartbeat_id": heartbeat_id });
        let value = self
            .raw_clob_json(reqwest::Method::POST, "v1/heartbeats", &[], Some(body))
            .await?;
        json_string_field(&value, "heartbeat_id").ok_or_else(|| {
            AppError::dependency_unavailable(
                "POLYMARKET_HEARTBEAT_RESPONSE_INVALID",
                "Polymarket heartbeat response did not include heartbeat_id",
            )
        })
    }

    async fn raw_reward_total_earnings_for_day_usd(
        &self,
        date: chrono::NaiveDate,
        include_sponsored: bool,
    ) -> Result<Decimal> {
        let mut query = vec![
            ("date", date.to_string()),
            ("signature_type", signature_type_query(self.signature_type)),
        ];
        if include_sponsored {
            query.push(("sponsored", "true".to_string()));
        }

        let value = self
            .raw_clob_json(reqwest::Method::GET, "rewards/user/total", &query, None)
            .await?;
        Ok(sum_reward_earnings_json_usd(&value))
    }

    async fn raw_reward_detailed_earnings_for_day_usd(
        &self,
        date: chrono::NaiveDate,
        sponsored_only: bool,
    ) -> Result<Decimal> {
        let mut next_cursor: Option<String> = None;
        let mut total = Decimal::ZERO;

        for _ in 0..CLOB_MAX_PAGES {
            let mut query = vec![
                ("date", date.to_string()),
                ("signature_type", signature_type_query(self.signature_type)),
            ];
            if sponsored_only {
                query.push(("sponsored", "true".to_string()));
            }
            if let Some(cursor) = &next_cursor {
                query.push(("next_cursor", cursor.clone()));
            }

            let value = self
                .raw_clob_json(reqwest::Method::GET, "rewards/user", &query, None)
                .await?;
            total += sum_reward_earnings_json_usd(&value);

            let Some(next) = json_string_field(&value, "next_cursor") else {
                break;
            };
            let count = value
                .get("count")
                .and_then(serde_json::Value::as_u64)
                .unwrap_or_default();
            if clob_page_is_terminal(&next, count, next_cursor.as_deref()) {
                break;
            }
            next_cursor = Some(next);
        }

        Ok(total.normalize())
    }

    async fn raw_clob_json(
        &self,
        method: reqwest::Method,
        path: &str,
        query: &[(&str, String)],
        body: Option<serde_json::Value>,
    ) -> Result<serde_json::Value> {
        let url = self.client.host().join(path).map_err(|error| {
            AppError::internal(
                "POLYMARKET_RAW_URL_INVALID",
                format!("failed to build Polymarket raw URL for {path}: {error}"),
            )
        })?;
        let client = reqwest::Client::new();
        let body_string = body.map(|body| body.to_string()).unwrap_or_default();
        let timestamp = chrono::Utc::now().timestamp();
        let mut request = client.request(method.clone(), url).query(query);
        if !body_string.is_empty() {
            request = request
                .header(reqwest::header::CONTENT_TYPE, "application/json")
                .body(body_string.clone());
        }
        let mut request = request.build().map_err(|error| {
            AppError::internal(
                "POLYMARKET_RAW_REQUEST_BUILD_FAILED",
                format!("failed to build Polymarket raw request: {error}"),
            )
        })?;
        let headers = self.l2_headers(
            method.as_str(),
            request.url().path(),
            &body_string,
            timestamp,
        )?;
        request.headers_mut().extend(headers);

        let response = client.execute(request).await.map_err(|error| {
            AppError::dependency_unavailable(
                "POLYMARKET_RAW_REQUEST_FAILED",
                format!("failed to send Polymarket raw request: {error}"),
            )
        })?;
        let status = response.status();
        let body = response.text().await.map_err(|error| {
            AppError::dependency_unavailable(
                "POLYMARKET_RAW_RESPONSE_READ_FAILED",
                format!("failed to read Polymarket raw response: {error}"),
            )
        })?;
        if !status.is_success() {
            return Err(AppError::dependency_unavailable(
                "POLYMARKET_RAW_STATUS_FAILED",
                format!("Polymarket raw request failed with HTTP {status}: {body}"),
            ));
        }
        parse_first_json_value(&body)
    }

    fn l2_headers(
        &self,
        method: &str,
        path: &str,
        body: &str,
        timestamp: i64,
    ) -> Result<reqwest::header::HeaderMap> {
        let credentials = self.client.credentials();
        let signature = l2_hmac_signature(
            credentials.secret().expose_secret(),
            &format!("{timestamp}{method}{path}{body}"),
        )?;
        let mut headers = reqwest::header::HeaderMap::new();
        insert_header(
            &mut headers,
            "POLY_ADDRESS",
            self.client.address().to_checksum(None),
        )?;
        insert_header(&mut headers, "POLY_API_KEY", credentials.key().to_string())?;
        insert_header(
            &mut headers,
            "POLY_PASSPHRASE",
            credentials.passphrase().expose_secret().to_string(),
        )?;
        insert_header(&mut headers, "POLY_SIGNATURE", signature)?;
        insert_header(&mut headers, "POLY_TIMESTAMP", timestamp.to_string())?;
        Ok(headers)
    }
}
