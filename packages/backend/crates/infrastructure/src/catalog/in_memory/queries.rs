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

async fn market_event_list_market_categories(&self) -> Result<Vec<MarketCategoryView>> {
        Ok(vec![
            MarketCategoryView { id: "sports".into(), label: "Sports".into(), sort_order: 1 },
            MarketCategoryView { id: "politics".into(), label: "Politics".into(), sort_order: 2 },
            MarketCategoryView { id: "crypto".into(), label: "Crypto".into(), sort_order: 3 },
            MarketCategoryView { id: "esports".into(), label: "Esports".into(), sort_order: 4 },
            MarketCategoryView { id: "finance".into(), label: "Finance".into(), sort_order: 5 },
            MarketCategoryView { id: "geopolitics".into(), label: "Geopolitics".into(), sort_order: 6 },
            MarketCategoryView { id: "tech".into(), label: "Tech".into(), sort_order: 7 },
            MarketCategoryView { id: "culture".into(), label: "Culture".into(), sort_order: 8 },
            MarketCategoryView { id: "economy".into(), label: "Economy".into(), sort_order: 9 },
            MarketCategoryView { id: "weather".into(), label: "Weather".into(), sort_order: 10 },
            MarketCategoryView { id: "pop_culture".into(), label: "Pop Culture".into(), sort_order: 11 },
            MarketCategoryView { id: "ai".into(), label: "AI".into(), sort_order: 12 },
            MarketCategoryView { id: "elections".into(), label: "Elections".into(), sort_order: 13 },
        ])
    }

async fn market_event_get_market(&self, market_id: &str) -> Result<Option<MarketView>> {
        Ok(self.markets.read().await.get(market_id).cloned())
    }

async fn market_event_get_markets_by_ids(&self, market_ids: &[String]) -> Result<Vec<MarketView>> {
        let markets = self.markets.read().await;
        Ok(market_ids
            .iter()
            .filter_map(|market_id| markets.get(market_id).cloned())
            .collect())
    }

async fn market_event_get_signal(&self, signal_id: &str) -> Result<Option<SignalView>> {
        Ok(self.signals.read().await.get(signal_id).cloned())
    }

async fn market_event_list_events(&self, filters: &EventListFilters, page: &PageQuery) -> Result<Paginated<EventView>> {
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
        let total = i64::try_from(items.len()).unwrap_or(i64::MAX);
        let offset = page.offset() as usize;
        let page_size = page.validated().1 as usize;
        let paged: Vec<_> = items.into_iter().skip(offset).take(page_size).collect();
        Ok(Paginated::new(paged, page, total))
    }

async fn market_event_list_evidences(&self, filters: &EvidenceListFilters, page: &PageQuery) -> Result<Paginated<EvidenceView>> {
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
        let total = i64::try_from(items.len()).unwrap_or(i64::MAX);
        let offset = page.offset() as usize;
        let page_size = page.validated().1 as usize;
        let paged: Vec<_> = items.into_iter().skip(offset).take(page_size).collect();
        Ok(Paginated::new(paged, page, total))
    }

async fn market_event_list_signals(&self, filters: &SignalListFilters, page: &PageQuery) -> Result<Paginated<SignalView>> {
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
        let total = i64::try_from(items.len()).unwrap_or(i64::MAX);
        let offset = page.offset() as usize;
        let page_size = page.validated().1 as usize;
        let paged: Vec<_> = items.into_iter().skip(offset).take(page_size).collect();
        Ok(Paginated::new(paged, page, total))
    }

async fn market_event_list_probability_estimates(
        &self,
        filters: &ProbabilityEstimateListFilters,
        page: &PageQuery,
    ) -> Result<Paginated<ProbabilityEstimateView>> {
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
        let total = i64::try_from(items.len()).unwrap_or(i64::MAX);
        let offset = page.offset() as usize;
        let page_size = page.validated().1 as usize;
        let paged: Vec<_> = items.into_iter().skip(offset).take(page_size).collect();
        Ok(Paginated::new(paged, page, total))
    }

async fn market_event_list_signal_transitions(
        &self,
        filters: &SignalTransitionListFilters,
        page: &PageQuery,
    ) -> Result<Paginated<SignalTransitionView>> {
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
        let total = i64::try_from(items.len()).unwrap_or(i64::MAX);
        let offset = page.offset() as usize;
        let page_size = page.validated().1 as usize;
        let paged: Vec<_> = items.into_iter().skip(offset).take(page_size).collect();
        Ok(Paginated::new(paged, page, total))
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

async fn market_event_list_active_order_market_ids(
    &self,
    connector_name: &str,
    limit: usize,
) -> Result<Vec<String>> {
    let orders = self.orders.read().await;
    let mut latest_by_market = HashMap::new();
    for order in orders.values().filter(|order| {
        order.connector_name == connector_name
            && matches!(
                order.status,
                OrderStatus::Submitted | OrderStatus::Open | OrderStatus::PartiallyFilled
            )
    }) {
        latest_by_market
            .entry(order.market_id.clone())
            .and_modify(|updated_at: &mut OffsetDateTime| {
                *updated_at = (*updated_at).max(order.updated_at);
            })
            .or_insert(order.updated_at);
    }
    let mut markets = latest_by_market.into_iter().collect::<Vec<_>>();
    markets.sort_by(|(left_id, left_at), (right_id, right_at)| {
        right_at.cmp(left_at).then_with(|| left_id.cmp(right_id))
    });
    markets.truncate(limit);
    Ok(markets.into_iter().map(|(market_id, _)| market_id).collect())
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
    async fn market_event_count_order_drafts(&self, filters: &OrderDraftListFilters) -> Result<i64> {
        let drafts = self.order_drafts.read().await;
        Ok(drafts.values().filter(|d| {
            filters.signal_id.as_ref().is_none_or(|s| &d.signal_id == s)
                && filters.connector_name.as_ref().is_none_or(|c| &d.connector_name == c)
                && filters.status.is_none_or(|s| d.status == s)
        }).count() as i64)
    }
    async fn market_event_list_order_drafts_paginated(&self, filters: &OrderDraftListFilters, page: &PageQuery) -> Result<Paginated<OrderDraftView>> {
        let drafts = self.order_drafts.read().await;
        let mut items: Vec<_> = drafts.values().filter(|d| {
            filters.signal_id.as_ref().is_none_or(|s| &d.signal_id == s)
                && filters.connector_name.as_ref().is_none_or(|c| &d.connector_name == c)
                && filters.status.is_none_or(|s| d.status == s)
        }).cloned().collect();
        let total = items.len() as i64;
        items.sort_by(|a, b| b.created_at.cmp(&a.created_at).then_with(|| a.id.cmp(&b.id)));
        let off = usize::try_from(page.offset()).unwrap_or(usize::MAX);
        let (_, ps) = page.validated();
        let off = usize::min(off, items.len());
        items = items.into_iter().skip(off).take(usize::from(ps)).collect();
        Ok(Paginated::new(items, page, total))
    }
    async fn market_event_count_execution_requests(&self, filters: &ExecutionRequestListFilters) -> Result<i64> {
        let reqs = self.execution_requests.read().await;
        Ok(reqs.values().filter(|r| {
            filters.signal_id.as_ref().is_none_or(|s| &r.signal_id == s)
                && filters.connector_name.as_ref().is_none_or(|c| &r.connector_name == c)
                && filters.status.is_none_or(|s| r.status == s)
        }).count() as i64)
    }
    async fn market_event_list_execution_requests_paginated(&self, filters: &ExecutionRequestListFilters, page: &PageQuery) -> Result<Paginated<ExecutionRequestView>> {
        let reqs = self.execution_requests.read().await;
        let mut items: Vec<_> = reqs.values().filter(|r| {
            filters.signal_id.as_ref().is_none_or(|s| &r.signal_id == s)
                && filters.connector_name.as_ref().is_none_or(|c| &r.connector_name == c)
                && filters.status.is_none_or(|s| r.status == s)
        }).cloned().collect();
        let total = items.len() as i64;
        items.sort_by(|a, b| b.created_at.cmp(&a.created_at).then_with(|| a.id.cmp(&b.id)));
        let off = usize::try_from(page.offset()).unwrap_or(usize::MAX);
        let (_, ps) = page.validated();
        let off = usize::min(off, items.len());
        items = items.into_iter().skip(off).take(usize::from(ps)).collect();
        Ok(Paginated::new(items, page, total))
    }
    async fn market_event_count_orders(&self, filters: &OrderListFilters) -> Result<i64> {
        let orders = self.orders.read().await;
        Ok(orders.values().filter(|o| {
            filters.signal_id.as_ref().is_none_or(|s| &o.signal_id == s)
                && filters.market_id.as_ref().is_none_or(|m| &o.market_id == m)
                && filters.connector_name.as_ref().is_none_or(|c| &o.connector_name == c)
                && filters.status.is_none_or(|s| o.status == s)
        }).count() as i64)
    }
    async fn market_event_list_orders_paginated(&self, filters: &OrderListFilters, page: &PageQuery) -> Result<Paginated<OrderView>> {
        let orders = self.orders.read().await;
        let mut items: Vec<_> = orders.values().filter(|o| {
            filters.signal_id.as_ref().is_none_or(|s| &o.signal_id == s)
                && filters.market_id.as_ref().is_none_or(|m| &o.market_id == m)
                && filters.connector_name.as_ref().is_none_or(|c| &o.connector_name == c)
                && filters.status.is_none_or(|s| o.status == s)
        }).cloned().collect();
        let total = items.len() as i64;
        items.sort_by(|a, b| b.updated_at.cmp(&a.updated_at).then_with(|| a.id.cmp(&b.id)));
        let off = usize::try_from(page.offset()).unwrap_or(usize::MAX);
        let (_, ps) = page.validated();
        let off = usize::min(off, items.len());
        items = items.into_iter().skip(off).take(usize::from(ps)).collect();
        Ok(Paginated::new(items, page, total))
    }
    async fn market_event_count_trades(&self, filters: &TradeListFilters) -> Result<i64> {
        let trades = self.trades.read().await;
        Ok(trades.values().filter(|t| {
            filters.order_id.as_ref().is_none_or(|o| &t.order_id == o)
                && filters.signal_id.as_ref().is_none_or(|s| &t.signal_id == s)
                && filters.market_id.as_ref().is_none_or(|m| &t.market_id == m)
                && filters.connector_name.as_ref().is_none_or(|c| &t.connector_name == c)
        }).count() as i64)
    }
    async fn market_event_list_trades_paginated(&self, filters: &TradeListFilters, page: &PageQuery) -> Result<Paginated<TradeView>> {
        let trades = self.trades.read().await;
        let mut items: Vec<_> = trades.values().filter(|t| {
            filters.order_id.as_ref().is_none_or(|o| &t.order_id == o)
                && filters.signal_id.as_ref().is_none_or(|s| &t.signal_id == s)
                && filters.market_id.as_ref().is_none_or(|m| &t.market_id == m)
                && filters.connector_name.as_ref().is_none_or(|c| &t.connector_name == c)
        }).cloned().collect();
        let total = items.len() as i64;
        items.sort_by(|a, b| b.executed_at.cmp(&a.executed_at).then_with(|| a.id.cmp(&b.id)));
        let off = usize::try_from(page.offset()).unwrap_or(usize::MAX);
        let (_, ps) = page.validated();
        let off = usize::min(off, items.len());
        items = items.into_iter().skip(off).take(usize::from(ps)).collect();
        Ok(Paginated::new(items, page, total))
    }
    async fn market_event_count_positions(&self, filters: &PositionListFilters) -> Result<i64> {
        let positions = self.positions.read().await;
        Ok(positions.values().filter(|p| {
            filters.market_id.as_ref().is_none_or(|m| &p.market_id == m)
                && filters.connector_name.as_ref().is_none_or(|c| &p.connector_name == c)
                && filters.side.is_none_or(|s| p.side == s)
        }).count() as i64)
    }
    async fn market_event_list_positions_paginated(&self, filters: &PositionListFilters, page: &PageQuery) -> Result<Paginated<PositionView>> {
        let positions = self.positions.read().await;
        let mut items: Vec<_> = positions.values().filter(|p| {
            filters.market_id.as_ref().is_none_or(|m| &p.market_id == m)
                && filters.connector_name.as_ref().is_none_or(|c| &p.connector_name == c)
                && filters.side.is_none_or(|s| p.side == s)
        }).cloned().collect();
        let total = items.len() as i64;
        items.sort_by(|a, b| b.updated_at.cmp(&a.updated_at).then_with(|| a.id.cmp(&b.id)));
        let off = usize::try_from(page.offset()).unwrap_or(usize::MAX);
        let (_, ps) = page.validated();
        let off = usize::min(off, items.len());
        items = items.into_iter().skip(off).take(usize::from(ps)).collect();
        Ok(Paginated::new(items, page, total))
    }
}
