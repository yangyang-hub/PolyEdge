impl InMemoryMarketEventStore {
async fn market_event_list_markets(&self, filters: &MarketListFilters) -> Result<Vec<MarketView>> {
        let markets = self.markets.read().await;
        let mut items: Vec<_> = markets
            .values()
            .filter(|market| {
                filters.status.is_none_or(|status| market.status == status)
                    && filters
                        .tradability_status
                        .is_none_or(|status| market.tradability_status == status)
                    && filters
                        .category
                        .as_ref()
                        .is_none_or(|cat| &market.category == cat)
            })
            .cloned()
            .collect();
        items.sort_by(|left, right| {
            let ord = match filters.sort_by {
                MarketSortField::Volume24h => left.volume_24h.cmp(&right.volume_24h),
                MarketSortField::UpdatedAt => left.updated_at.cmp(&right.updated_at),
            };
            let ord = match filters.sort_order {
                SortOrder::Asc => ord,
                SortOrder::Desc => ord.reverse(),
            };
            ord.then_with(|| left.id.cmp(&right.id))
        });
        let offset = usize::min(filters.offset as usize, items.len());
        items = items.into_iter().skip(offset).take(usize::from(filters.limit)).collect();
        Ok(items)
    }

async fn market_event_count_markets(&self, filters: &MarketListFilters) -> Result<i64> {
        let markets = self.markets.read().await;
        let count = markets
            .values()
            .filter(|market| {
                filters.status.is_none_or(|status| market.status == status)
                    && filters
                        .tradability_status
                        .is_none_or(|status| market.tradability_status == status)
                    && filters
                        .category
                        .as_ref()
                        .is_none_or(|cat| &market.category == cat)
            })
            .count();
        Ok(count as i64)
    }

async fn market_event_get_market(&self, market_id: &str) -> Result<Option<MarketView>> {
        Ok(self.markets.read().await.get(market_id).cloned())
    }

async fn market_event_get_signal(&self, signal_id: &str) -> Result<Option<SignalView>> {
        Ok(self.signals.read().await.get(signal_id).cloned())
    }

async fn market_event_list_events(&self, filters: &EventListFilters) -> Result<Vec<EventView>> {
        let events = self.events.read().await;
        let mut items: Vec<_> = events
            .values()
            .filter(|event| filters.status.is_none_or(|status| event.status == status))
            .cloned()
            .collect();
        items.sort_by(|left, right| {
            right
                .updated_at
                .cmp(&left.updated_at)
                .then_with(|| left.id.cmp(&right.id))
        });
        items.truncate(usize::from(filters.limit));
        Ok(items)
    }

async fn market_event_list_evidences(&self, filters: &EvidenceListFilters) -> Result<Vec<EvidenceView>> {
        let evidences = self.evidences.read().await;
        let mut items: Vec<_> = evidences
            .values()
            .filter(|evidence| {
                filters
                    .market_id
                    .as_ref()
                    .is_none_or(|market_id| &evidence.market_id == market_id)
                    && filters
                        .event_id
                        .as_ref()
                        .is_none_or(|event_id| &evidence.event_id == event_id)
                    && filters
                        .status
                        .is_none_or(|status| evidence.status == status)
            })
            .cloned()
            .collect();
        items.sort_by(|left, right| {
            right
                .created_at
                .cmp(&left.created_at)
                .then_with(|| left.id.cmp(&right.id))
        });
        items.truncate(usize::from(filters.limit));
        Ok(items)
    }

async fn market_event_list_signals(&self, filters: &SignalListFilters) -> Result<Vec<SignalView>> {
        let signals = self.signals.read().await;
        let mut items: Vec<_> = signals
            .values()
            .filter(|signal| {
                filters
                    .market_id
                    .as_ref()
                    .is_none_or(|market_id| &signal.market_id == market_id)
                    && filters
                        .event_id
                        .as_ref()
                        .is_none_or(|event_id| &signal.event_id == event_id)
                    && filters
                        .lifecycle_state
                        .is_none_or(|state| signal.lifecycle_state == state)
            })
            .cloned()
            .collect();
        items.sort_by(|left, right| {
            right
                .updated_at
                .cmp(&left.updated_at)
                .then_with(|| left.id.cmp(&right.id))
        });
        items.truncate(usize::from(filters.limit));
        Ok(items)
    }

async fn market_event_list_probability_estimates(
        &self,
        filters: &ProbabilityEstimateListFilters,
    ) -> Result<Vec<ProbabilityEstimateView>> {
        let estimates = self.probability_estimates.read().await;
        let mut items: Vec<_> = estimates
            .values()
            .filter(|estimate| {
                filters
                    .market_id
                    .as_ref()
                    .is_none_or(|market_id| &estimate.market_id == market_id)
                    && filters
                        .event_id
                        .as_ref()
                        .is_none_or(|event_id| &estimate.event_id == event_id)
                    && filters
                        .signal_id
                        .as_ref()
                        .is_none_or(|signal_id| estimate.signal_id.as_ref() == Some(signal_id))
            })
            .cloned()
            .collect();
        items.sort_by(|left, right| {
            right
                .created_at
                .cmp(&left.created_at)
                .then_with(|| left.id.cmp(&right.id))
        });
        items.truncate(usize::from(filters.limit));
        Ok(items)
    }

async fn market_event_list_signal_transitions(
        &self,
        filters: &SignalTransitionListFilters,
    ) -> Result<Vec<SignalTransitionView>> {
        let transitions = self.signal_transitions.read().await;
        let mut items: Vec<_> = transitions
            .iter()
            .filter(|transition| transition.signal_id == filters.signal_id)
            .cloned()
            .collect();
        items.sort_by(|left, right| {
            right
                .created_at
                .cmp(&left.created_at)
                .then_with(|| left.id.cmp(&right.id))
        });
        items.truncate(usize::from(filters.limit));
        Ok(items)
    }

async fn market_event_list_order_drafts(
        &self,
        filters: &OrderDraftListFilters,
    ) -> Result<Vec<OrderDraftView>> {
        let order_drafts = self.order_drafts.read().await;
        let mut items: Vec<_> = order_drafts
            .values()
            .filter(|draft| {
                filters
                    .signal_id
                    .as_ref()
                    .is_none_or(|signal_id| &draft.signal_id == signal_id)
                    && filters
                        .connector_name
                        .as_ref()
                        .is_none_or(|connector_name| &draft.connector_name == connector_name)
                    && filters.status.is_none_or(|status| draft.status == status)
            })
            .cloned()
            .collect();
        items.sort_by(|left, right| {
            right
                .created_at
                .cmp(&left.created_at)
                .then_with(|| left.id.cmp(&right.id))
        });
        items.truncate(usize::from(filters.limit));
        Ok(items)
    }

async fn market_event_list_execution_requests(
        &self,
        filters: &ExecutionRequestListFilters,
    ) -> Result<Vec<ExecutionRequestView>> {
        let execution_requests = self.execution_requests.read().await;
        let mut items: Vec<_> = execution_requests
            .values()
            .filter(|request| {
                filters
                    .signal_id
                    .as_ref()
                    .is_none_or(|signal_id| &request.signal_id == signal_id)
                    && filters
                        .connector_name
                        .as_ref()
                        .is_none_or(|connector_name| &request.connector_name == connector_name)
                    && filters.status.is_none_or(|status| request.status == status)
            })
            .cloned()
            .collect();
        items.sort_by(|left, right| {
            right
                .created_at
                .cmp(&left.created_at)
                .then_with(|| left.id.cmp(&right.id))
        });
        items.truncate(usize::from(filters.limit));
        Ok(items)
    }

async fn market_event_get_order_by_external_ref(
        &self,
        connector_name: &str,
        external_order_id: &str,
    ) -> Result<OrderView> {
        let orders = self.orders.read().await;
        orders
            .values()
            .find(|order| {
                order.connector_name == connector_name
                    && order.external_order_id == external_order_id
            })
            .cloned()
            .ok_or_else(|| {
                AppError::not_found(
                    "ORDER_NOT_FOUND",
                    format!(
                        "order was not found for connector={} external_order_id={}",
                        connector_name, external_order_id
                    ),
                )
            })
    }

async fn market_event_list_orders(&self, filters: &OrderListFilters) -> Result<Vec<OrderView>> {
        let orders = self.orders.read().await;
        let mut items: Vec<_> = orders
            .values()
            .filter(|order| {
                filters
                    .signal_id
                    .as_ref()
                    .is_none_or(|signal_id| &order.signal_id == signal_id)
                    && filters
                        .market_id
                        .as_ref()
                        .is_none_or(|market_id| &order.market_id == market_id)
                    && filters
                        .connector_name
                        .as_ref()
                        .is_none_or(|connector_name| &order.connector_name == connector_name)
                    && filters.status.is_none_or(|status| order.status == status)
            })
            .cloned()
            .collect();
        items.sort_by(|left, right| {
            right
                .updated_at
                .cmp(&left.updated_at)
                .then_with(|| left.id.cmp(&right.id))
        });
        items.truncate(usize::from(filters.limit));
        Ok(items)
    }

async fn market_event_list_trades(&self, filters: &TradeListFilters) -> Result<Vec<TradeView>> {
        let trades = self.trades.read().await;
        let mut items: Vec<_> = trades
            .values()
            .filter(|trade| {
                filters
                    .order_id
                    .as_ref()
                    .is_none_or(|order_id| &trade.order_id == order_id)
                    && filters
                        .signal_id
                        .as_ref()
                        .is_none_or(|signal_id| &trade.signal_id == signal_id)
                    && filters
                        .market_id
                        .as_ref()
                        .is_none_or(|market_id| &trade.market_id == market_id)
                    && filters
                        .connector_name
                        .as_ref()
                        .is_none_or(|connector_name| &trade.connector_name == connector_name)
            })
            .cloned()
            .collect();
        items.sort_by(|left, right| {
            right
                .executed_at
                .cmp(&left.executed_at)
                .then_with(|| left.id.cmp(&right.id))
        });
        items.truncate(usize::from(filters.limit));
        Ok(items)
    }

async fn market_event_list_positions(&self, filters: &PositionListFilters) -> Result<Vec<PositionView>> {
        let positions = self.positions.read().await;
        let mut items: Vec<_> = positions
            .values()
            .filter(|position| {
                filters
                    .market_id
                    .as_ref()
                    .is_none_or(|market_id| &position.market_id == market_id)
                    && filters
                        .connector_name
                        .as_ref()
                        .is_none_or(|connector_name| &position.connector_name == connector_name)
                    && filters.side.is_none_or(|side| position.side == side)
            })
            .cloned()
            .collect();
        items.sort_by(|left, right| {
            right
                .updated_at
                .cmp(&left.updated_at)
                .then_with(|| left.id.cmp(&right.id))
        });
        items.truncate(usize::from(filters.limit));
        Ok(items)
    }
}
