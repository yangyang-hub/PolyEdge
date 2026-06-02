#[async_trait]
pub trait ArbitrageStore: Send + Sync {
    async fn start_arbitrage_scan(&self, scan: &ArbitrageScanView) -> Result<()>;

    async fn complete_arbitrage_scan(
        &self,
        scan_id: &str,
        finished_at: OffsetDateTime,
        market_count: u32,
        snapshot_count: u32,
        opportunity_count: u32,
    ) -> Result<ArbitrageScanView>;

    async fn record_market_book_snapshot(&self, snapshot: &MarketBookSnapshotView) -> Result<()>;

    async fn record_arbitrage_opportunity(
        &self,
        opportunity: &ArbitrageOpportunityView,
    ) -> Result<()>;

    async fn record_arbitrage_opportunity_validation(
        &self,
        validation: &ArbitrageOpportunityValidationView,
    ) -> Result<()>;

    async fn expire_arbitrage_opportunities(
        &self,
        observed_before: OffsetDateTime,
        trace_id: &str,
    ) -> Result<Vec<ArbitrageOpportunityView>>;

    async fn list_arbitrage_scans(
        &self,
        filters: &ArbitrageScanListFilters,
        page: &PageQuery,
    ) -> Result<Paginated<ArbitrageScanView>>;

    async fn list_arbitrage_opportunities(
        &self,
        filters: &ArbitrageOpportunityListFilters,
        page: &PageQuery,
    ) -> Result<Paginated<ArbitrageOpportunityView>>;

    async fn record_arbitrage_analysis_run(
        &self,
        analysis: &ArbitrageAnalysisRunView,
    ) -> Result<()>;

    async fn list_arbitrage_analysis_runs(
        &self,
        filters: &ArbitrageAnalysisRunListFilters,
        page: &PageQuery,
    ) -> Result<Paginated<ArbitrageAnalysisRunView>>;

    async fn record_arbitrage_event(
        &self,
        event: &ArbitrageEventView,
    ) -> Result<ArbitrageEventView>;

    async fn list_arbitrage_events(
        &self,
        filters: &ArbitrageEventListFilters,
        page: &PageQuery,
    ) -> Result<Paginated<ArbitrageEventView>>;

    async fn prune_arbitrage_events(&self, occurred_before: OffsetDateTime) -> Result<u64>;
}

pub struct ArbitrageService {
    store: Arc<dyn ArbitrageStore>,
}

impl ArbitrageService {
    #[must_use]
    pub fn new(store: Arc<dyn ArbitrageStore>) -> Self {
        Self { store }
    }

    pub async fn start_scan(&self, scan: ArbitrageScanView) -> Result<ArbitrageScanView> {
        self.store.start_arbitrage_scan(&scan).await?;
        self.record_event(
            ArbitrageEventType::ScanStarted,
            "scan",
            &scan.id,
            scan_payload(&scan),
            scan.started_at,
            &scan.trace_id,
        )
        .await?;
        Ok(scan)
    }

    pub async fn complete_scan(
        &self,
        scan_id: &str,
        finished_at: OffsetDateTime,
        market_count: u32,
        snapshot_count: u32,
        opportunity_count: u32,
    ) -> Result<ArbitrageScanView> {
        let scan = self
            .store
            .complete_arbitrage_scan(
                scan_id,
                finished_at,
                market_count,
                snapshot_count,
                opportunity_count,
            )
            .await?;
        self.record_event(
            ArbitrageEventType::ScanCompleted,
            "scan",
            &scan.id,
            scan_payload(&scan),
            finished_at,
            &scan.trace_id,
        )
        .await?;
        Ok(scan)
    }

    pub async fn record_snapshot_and_detect(
        &self,
        snapshot: MarketBookSnapshotView,
    ) -> Result<Vec<ArbitrageOpportunityView>> {
        self.store.record_market_book_snapshot(&snapshot).await?;
        let drafts = detect_arbitrage_opportunities(&snapshot)?;
        let mut opportunities = Vec::with_capacity(drafts.len());

        for draft in drafts {
            let repeated = self
                .is_repeated_opportunity(
                    &snapshot.market_id,
                    draft.opportunity_type,
                    snapshot.observed_at,
                )
                .await?;
            let status = if repeated {
                ArbitrageOpportunityStatus::Repeated
            } else {
                ArbitrageOpportunityStatus::Observed
            };
            let opportunity = ArbitrageOpportunityView {
                id: opportunity_id(
                    &snapshot.scan_id,
                    &snapshot.market_id,
                    draft.opportunity_type,
                ),
                scan_id: snapshot.scan_id.clone(),
                market_id: snapshot.market_id.clone(),
                opportunity_type: draft.opportunity_type,
                status,
                gross_edge: draft.gross_edge,
                price_sum: draft.price_sum,
                capacity: draft.capacity,
                yes_price: draft.yes_price,
                no_price: draft.no_price,
                yes_size: draft.yes_size,
                no_size: draft.no_size,
                observed_at: snapshot.observed_at,
                reason_codes: draft.reason_codes,
                analysis_payload: draft.analysis_payload,
                trace_id: snapshot.trace_id.clone(),
                validation: None,
            };
            self.store
                .record_arbitrage_opportunity(&opportunity)
                .await?;
            self.record_event(
                if repeated {
                    ArbitrageEventType::OpportunityRepeated
                } else {
                    ArbitrageEventType::OpportunityObserved
                },
                "opportunity",
                &opportunity.id,
                opportunity_payload(&opportunity),
                opportunity.observed_at,
                &opportunity.trace_id,
            )
            .await?;
            opportunities.push(opportunity);
        }

        Ok(opportunities)
    }

    pub async fn record_book_snapshot(
        &self,
        snapshot: MarketBookSnapshotView,
    ) -> Result<MarketBookSnapshotView> {
        self.store.record_market_book_snapshot(&snapshot).await?;
        Ok(snapshot)
    }

    pub async fn validate_opportunity(
        &self,
        opportunity: &ArbitrageOpportunityView,
        snapshot: &MarketBookSnapshotView,
        config: &ArbitrageValidationConfig,
        validated_at: OffsetDateTime,
    ) -> Result<ArbitrageOpportunityValidationView> {
        let validation = validate_arbitrage_opportunity(
            opportunity,
            snapshot,
            config,
            validated_at,
            &opportunity.trace_id,
        )?;
        self.store
            .record_arbitrage_opportunity_validation(&validation)
            .await?;
        self.record_event(
            if validation.status == ArbitrageValidationStatus::Valid {
                ArbitrageEventType::ValidationPassed
            } else {
                ArbitrageEventType::ValidationFailed
            },
            "validation",
            &validation.opportunity_id,
            validation_payload(&validation),
            validation.validated_at,
            &validation.trace_id,
        )
        .await?;

        Ok(validation)
    }

    pub async fn expire_opportunities(
        &self,
        observed_before: OffsetDateTime,
        trace_id: &str,
    ) -> Result<Vec<ArbitrageOpportunityView>> {
        let expired = self
            .store
            .expire_arbitrage_opportunities(observed_before, trace_id)
            .await?;

        for opportunity in &expired {
            self.record_event(
                ArbitrageEventType::OpportunityExpired,
                "opportunity",
                &opportunity.id,
                opportunity_payload(opportunity),
                OffsetDateTime::now_utc(),
                trace_id,
            )
            .await?;
        }

        Ok(expired)
    }

    pub async fn list_opportunities(
        &self,
        filters: ArbitrageOpportunityListFilters,
        page: &PageQuery,
    ) -> Result<Paginated<ArbitrageOpportunityView>> {
        self.store.list_arbitrage_opportunities(&filters, page).await
    }

    pub async fn list_scans(
        &self,
        filters: ArbitrageScanListFilters,
        page: &PageQuery,
    ) -> Result<Paginated<ArbitrageScanView>> {
        self.store.list_arbitrage_scans(&filters, page).await
    }

    pub async fn record_analysis_run(
        &self,
        analysis: ArbitrageAnalysisRunView,
    ) -> Result<ArbitrageAnalysisRunView> {
        self.store.record_arbitrage_analysis_run(&analysis).await?;
        self.record_event(
            ArbitrageEventType::AnalysisGenerated,
            "analysis",
            &analysis.id,
            analysis_payload(&analysis),
            analysis.generated_at,
            &analysis.trace_id,
        )
        .await?;
        Ok(analysis)
    }

    pub async fn list_analysis_runs(
        &self,
        filters: ArbitrageAnalysisRunListFilters,
        page: &PageQuery,
    ) -> Result<Paginated<ArbitrageAnalysisRunView>> {
        self.store.list_arbitrage_analysis_runs(&filters, page).await
    }

    pub async fn list_events(
        &self,
        filters: ArbitrageEventListFilters,
        page: &PageQuery,
    ) -> Result<Paginated<ArbitrageEventView>> {
        self.store.list_arbitrage_events(&filters, page).await
    }

    pub async fn prune_events(&self, occurred_before: OffsetDateTime) -> Result<u64> {
        self.store.prune_arbitrage_events(occurred_before).await
    }

    async fn is_repeated_opportunity(
        &self,
        market_id: &str,
        opportunity_type: ArbitrageOpportunityType,
        observed_at: OffsetDateTime,
    ) -> Result<bool> {
        let repeated_after = observed_at - time::Duration::seconds(DEFAULT_REPEAT_WINDOW_SECONDS);
        let query = PageQuery { page: 1, page_size: 1, sort_order: None };
        let recent = self
            .store
            .list_arbitrage_opportunities(&ArbitrageOpportunityListFilters::new(
                Some(market_id.to_string()),
                Some(opportunity_type),
                None,
                None,
                None,
                Some(repeated_after),
                true,
            )?, &query)
            .await?;

        Ok(recent.data.into_iter().any(|opportunity| {
            opportunity.status != ArbitrageOpportunityStatus::Expired
                && opportunity.observed_at < observed_at
        }))
    }

    async fn record_event(
        &self,
        event_type: ArbitrageEventType,
        resource_type: &str,
        resource_id: &str,
        payload: Value,
        occurred_at: OffsetDateTime,
        trace_id: &str,
    ) -> Result<ArbitrageEventView> {
        let event = ArbitrageEventView {
            sequence: 0,
            id: arbitrage_event_id(event_type, resource_id, occurred_at),
            event_type,
            resource_type: resource_type.to_string(),
            resource_id: resource_id.to_string(),
            payload,
            occurred_at,
            trace_id: trace_id.to_string(),
        };

        self.store.record_arbitrage_event(&event).await
    }
}
