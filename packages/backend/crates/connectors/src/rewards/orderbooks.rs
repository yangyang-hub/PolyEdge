fn spawn_next_orderbook_fetch<'a>(
    tasks: &mut JoinSet<Result<Option<PolymarketRewardOrderBook>>>,
    pending: &mut std::slice::Iter<'a, String>,
    connector: &PolymarketRewardsConnector,
) {
    if let Some(token_id) = pending.next() {
        let connector = connector.clone();
        let token_id = token_id.clone();
        tasks.spawn(async move { connector.fetch_order_book(&token_id).await });
    }
}

impl PolymarketRewardsConnector {
    pub async fn fetch_order_books(
        &self,
        token_ids: &[String],
    ) -> Result<Vec<PolymarketRewardOrderBook>> {
        if token_ids.is_empty() {
            return Ok(Vec::new());
        }

        let mut books = match self.fetch_order_book_batch(token_ids).await {
            Ok(books) => books,
            Err(error) => {
                tracing::warn!(
                    error = %error,
                    requested = token_ids.len(),
                    "Polymarket order book batch failed, falling back to individual requests"
                );
                Vec::new()
            }
        };
        let fetched = books
            .iter()
            .map(|book| book.token_id.as_str())
            .collect::<HashSet<_>>();
        let missing = token_ids
            .iter()
            .filter(|token_id| !fetched.contains(token_id.as_str()))
            .cloned()
            .collect::<Vec<_>>();
        if !missing.is_empty() {
            match self.fetch_order_books_individually(&missing).await {
                Ok(fallback_books) => books.extend(fallback_books),
                Err(error) if books.is_empty() => return Err(error),
                Err(error) => {
                    tracing::warn!(
                        error = %error,
                        requested = missing.len(),
                        fetched = books.len(),
                        "individual Polymarket order book fallback failed; keeping batch results"
                    );
                }
            }
        }
        if books.is_empty() {
            return Err(AppError::dependency_unavailable(
                "POLYMARKET_BOOK_BATCH_EMPTY",
                "Polymarket order book requests returned no usable books",
            ));
        }
        Ok(books)
    }

    async fn fetch_order_book_batch(
        &self,
        token_ids: &[String],
    ) -> Result<Vec<PolymarketRewardOrderBook>> {
        let requests = token_ids
            .iter()
            .map(|token_id| serde_json::json!({ "token_id": token_id }))
            .collect::<Vec<_>>();
        let response = self
            .client
            .post(format!("{}/books", self.clob_host))
            .json(&requests)
            .send()
            .await
            .map_err(|error| {
                AppError::dependency_unavailable(
                    "POLYMARKET_BOOK_BATCH_REQUEST_FAILED",
                    format!("failed to request Polymarket order book batch: {error}"),
                )
            })?;
        let status = response.status();
        if !status.is_success() {
            return Err(AppError::dependency_unavailable(
                "POLYMARKET_BOOK_BATCH_FAILED",
                format!("Polymarket order book batch returned HTTP {status}"),
            ));
        }
        let raws = response
            .json::<Vec<RawOrderBook>>()
            .await
            .map_err(|error| {
                AppError::dependency_unavailable(
                    "POLYMARKET_BOOK_BATCH_DECODE_FAILED",
                    format!("failed to decode Polymarket order book batch: {error}"),
                )
            })?;
        let books = map_requested_reward_order_books(raws, token_ids);
        if books.len() < token_ids.len() {
            tracing::warn!(
                requested = token_ids.len(),
                fetched = books.len(),
                missing = token_ids.len() - books.len(),
                "Polymarket order book batch omitted requested books"
            );
        }
        Ok(books)
    }

    async fn fetch_order_books_individually(
        &self,
        token_ids: &[String],
    ) -> Result<Vec<PolymarketRewardOrderBook>> {
        let mut pending = token_ids.iter();
        let mut tasks = JoinSet::new();
        let mut books = Vec::new();
        let mut failures = 0usize;
        for _ in 0..ENRICH_CONCURRENCY {
            spawn_next_orderbook_fetch(&mut tasks, &mut pending, self);
        }
        while let Some(result) = tasks.join_next().await {
            match result {
                Ok(Ok(Some(book))) => books.push(book),
                Ok(Ok(None) | Err(_)) | Err(_) => failures += 1,
            }
            spawn_next_orderbook_fetch(&mut tasks, &mut pending, self);
        }
        if books.is_empty() && failures > 0 {
            return Err(AppError::dependency_unavailable(
                "POLYMARKET_BOOK_INDIVIDUAL_FALLBACK_FAILED",
                format!("failed to fetch all {failures} requested Polymarket order books"),
            ));
        }
        if failures > 0 {
            tracing::warn!(
                requested = token_ids.len(),
                fetched = books.len(),
                failures,
                "individual Polymarket order book fallback completed with failures"
            );
        }
        Ok(books)
    }

    async fn fetch_order_book(&self, token_id: &str) -> Result<Option<PolymarketRewardOrderBook>> {
        let mut url =
            reqwest::Url::parse(&format!("{}/book", self.clob_host)).map_err(|error| {
                AppError::invalid_input(
                    "POLYMARKET_BOOK_URL_INVALID",
                    format!("failed to construct order book URL: {error}"),
                )
            })?;
        url.query_pairs_mut().append_pair("token_id", token_id);
        let response = self.client.get(url).send().await.map_err(|error| {
            AppError::dependency_unavailable(
                "POLYMARKET_BOOK_REQUEST_FAILED",
                format!("failed to request Polymarket order book for token_id={token_id}: {error}"),
            )
        })?;
        if response.status() == reqwest::StatusCode::NOT_FOUND {
            return Ok(None);
        }
        let status = response.status();
        if !status.is_success() {
            return Err(AppError::dependency_unavailable(
                "POLYMARKET_BOOK_STATUS_FAILED",
                format!("Polymarket order book returned HTTP {status} for token_id={token_id}"),
            ));
        }
        response
            .json::<RawOrderBook>()
            .await
            .map_err(|error| {
                AppError::dependency_unavailable(
                    "POLYMARKET_BOOK_DECODE_FAILED",
                    format!(
                        "failed to decode Polymarket order book for token_id={token_id}: {error}"
                    ),
                )
            })
            .map(|raw| map_reward_order_book_with_fallback(raw, token_id))
    }
}

fn map_reward_order_book(raw: RawOrderBook) -> Option<PolymarketRewardOrderBook> {
    let token_id = raw.asset_id?.trim().to_string();
    if token_id.is_empty() {
        return None;
    }
    Some(PolymarketRewardOrderBook {
        token_id,
        bids: parse_levels(raw.bids, SortDirection::Descending),
        asks: parse_levels(raw.asks, SortDirection::Ascending),
        observed_at: OffsetDateTime::now_utc(),
    })
}

fn map_reward_order_book_with_fallback(
    mut raw: RawOrderBook,
    fallback_token_id: &str,
) -> Option<PolymarketRewardOrderBook> {
    if raw.asset_id.as_deref().is_none_or(str::is_empty) {
        raw.asset_id = Some(fallback_token_id.to_string());
    }
    map_reward_order_book(raw)
}

fn map_requested_reward_order_books(
    raws: Vec<RawOrderBook>,
    token_ids: &[String],
) -> Vec<PolymarketRewardOrderBook> {
    let requested = token_ids
        .iter()
        .map(String::as_str)
        .collect::<HashSet<_>>();
    let mut seen = HashSet::new();
    raws.into_iter()
        .filter_map(map_reward_order_book)
        .filter(|book| {
            requested.contains(book.token_id.as_str()) && seen.insert(book.token_id.clone())
        })
        .collect()
}

#[derive(Debug, Clone, Copy)]
enum SortDirection {
    Ascending,
    Descending,
}

fn parse_levels(
    levels: Option<Vec<RawBookLevel>>,
    direction: SortDirection,
) -> Vec<PolymarketRewardBookLevel> {
    let mut parsed = levels
        .unwrap_or_default()
        .into_iter()
        .filter_map(|level| {
            let price = parse_decimal(level.price.as_deref())?;
            let size = parse_decimal(level.size.as_deref())?;
            if size <= Decimal::ZERO {
                return None;
            }
            Some(PolymarketRewardBookLevel { price, size })
        })
        .collect::<Vec<_>>();

    parsed.sort_by(|left, right| match direction {
        SortDirection::Ascending => left.price.cmp(&right.price),
        SortDirection::Descending => right.price.cmp(&left.price),
    });
    parsed
}

fn parse_decimal(value: Option<&str>) -> Option<Decimal> {
    let raw = value?.trim();
    if raw.is_empty() {
        return None;
    }
    Decimal::from_str(raw).ok()
}

#[cfg(test)]
mod orderbook_tests {
    use super::*;

    #[test]
    fn batch_order_book_mapping_keeps_best_level_order() {
        let book = map_reward_order_book(RawOrderBook {
            asset_id: Some("123".to_string()),
            bids: Some(vec![
                RawBookLevel {
                    price: Some("0.40".to_string()),
                    size: Some("5".to_string()),
                },
                RawBookLevel {
                    price: Some("0.50".to_string()),
                    size: Some("7".to_string()),
                },
            ]),
            asks: Some(vec![
                RawBookLevel {
                    price: Some("0.70".to_string()),
                    size: Some("5".to_string()),
                },
                RawBookLevel {
                    price: Some("0.60".to_string()),
                    size: Some("7".to_string()),
                },
            ]),
        })
        .expect("mapped order book");

        assert_eq!(book.bids[0].price, Decimal::new(50, 2));
        assert_eq!(book.asks[0].price, Decimal::new(60, 2));
    }

    #[test]
    fn batch_order_book_mapping_deduplicates_and_filters_unrequested_books() {
        let raw = |token_id: &str| RawOrderBook {
            asset_id: Some(token_id.to_string()),
            bids: None,
            asks: None,
        };
        let books = map_requested_reward_order_books(
            vec![raw("requested"), raw("requested"), raw("unexpected")],
            &["requested".to_string()],
        );

        assert_eq!(books.len(), 1);
        assert_eq!(books[0].token_id, "requested");
    }
}
