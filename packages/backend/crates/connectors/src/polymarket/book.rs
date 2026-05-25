impl PolymarketBookConnector {
    pub fn new(clob_host: &str) -> Result<Self> {
        let client = ClobClient::new(clob_host, ClobConfig::default()).map_err(|error| {
            AppError::internal(
                "POLYMARKET_CLIENT_INIT_FAILED",
                format!("failed to initialize Polymarket CLOB book client: {error}"),
            )
        })?;

        Ok(Self { client })
    }

    pub async fn fetch_binary_book(
        &self,
        market_refs: &PolymarketMarketRefs,
    ) -> Result<PolymarketBinaryBookSnapshot> {
        let yes_asset_id = market_refs.asset_id_for_side(SignalSide::Yes)?;
        let no_asset_id = market_refs.asset_id_for_side(SignalSide::No)?;
        let yes = self.fetch_token_book(yes_asset_id).await?;
        let no = self.fetch_token_book(no_asset_id).await?;
        let observed_at = max_time(yes.observed_at, no.observed_at);

        Ok(PolymarketBinaryBookSnapshot {
            condition_id: market_refs.condition_id.clone(),
            yes_asset_id: market_refs.yes_asset_id.clone(),
            no_asset_id: market_refs.no_asset_id.clone(),
            yes,
            no,
            observed_at,
        })
    }

    async fn fetch_token_book(&self, asset_id: U256) -> Result<PolymarketSingleTokenBook> {
        let request = OrderBookSummaryRequest::builder()
            .token_id(asset_id)
            .build();
        let response = self.client.order_book(&request).await.map_err(|error| {
            AppError::dependency_unavailable(
                "POLYMARKET_ORDER_BOOK_QUERY_FAILED",
                format!("failed to query Polymarket order book for asset_id={asset_id}: {error}"),
            )
        })?;
        let observed_at = OffsetDateTime::now_utc();
        let raw_payload = serde_json::to_value(&response).map_err(|error| {
            AppError::internal(
                "POLYMARKET_ORDER_BOOK_ENCODE_FAILED",
                format!("failed to encode Polymarket order book for asset_id={asset_id}: {error}"),
            )
        })?;

        Ok(PolymarketSingleTokenBook {
            asset_id: asset_id.to_string(),
            best_bid: best_bid_level(response.bids)?,
            best_ask: best_ask_level(response.asks)?,
            raw_payload,
            observed_at,
        })
    }
}
