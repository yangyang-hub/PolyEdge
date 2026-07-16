use super::*;
use polyedge_connectors::{PolymarketOrderLifecycleStatus, PolymarketOrderSnapshot};

impl RuntimeSupervisor {
    pub(super) async fn verify_managed_orders(
        &self,
        connector: &LivePolymarketConnector,
        job: &WalletExecutionJob,
        managed_orders: &mut [ManagedOrder],
        venue_orders: &[PolymarketOpenOrder],
    ) -> Result<()> {
        let venue_by_id = venue_orders
            .iter()
            .map(|order| (order.id.as_str(), order))
            .collect::<HashMap<_, _>>();

        for order in managed_orders
            .iter_mut()
            .filter(|order| is_open_like_order_status(order.status))
        {
            let Some(external_order_id) = order.external_order_id.as_deref() else {
                self.store
                    .mark_order_unknown(job, order, "VENUE_ORDER_ID_MISSING")
                    .await?;
                return Err(AppError::conflict(
                    "EXECUTION_ORDER_EXTERNAL_ID_MISSING",
                    format!("managed order {} has no venue id", order.id),
                ));
            };
            let (snapshot, reason) = if let Some(venue_order) = venue_by_id.get(external_order_id) {
                (
                    PolymarketOrderSnapshot {
                        external_order_id: venue_order.id.clone(),
                        status: venue_order.lifecycle_status,
                        filled_quantity: venue_order.size_matched.max(Decimal::ZERO),
                    },
                    "VENUE_OPEN_ORDER_RECONCILED",
                )
            } else {
                match connector.order_snapshot(external_order_id).await {
                    Ok(snapshot) => (snapshot, "VENUE_ORDER_QUERY_RECONCILED"),
                    Err(error) => {
                        self.store
                            .mark_order_unknown(job, order, "VENUE_ORDER_QUERY_FAILED")
                            .await?;
                        return Err(error);
                    }
                }
            };
            if snapshot.external_order_id != external_order_id
                || snapshot.filled_quantity > order.quantity
            {
                self.store
                    .mark_order_unknown(job, order, "VENUE_ORDER_SNAPSHOT_INVALID")
                    .await?;
                return Err(AppError::conflict(
                    "EXECUTION_VENUE_ORDER_SNAPSHOT_INVALID",
                    format!(
                        "venue returned an invalid snapshot for managed order {}",
                        order.id
                    ),
                ));
            }

            let status = managed_order_status(snapshot.status);
            self.store
                .reconcile_managed_order(job, order, status, snapshot.filled_quantity, reason)
                .await?;
            order.status = status;
            order.filled_quantity = snapshot.filled_quantity;
            order.updated_at = OffsetDateTime::now_utc();

            if status == ManagedOrderStatus::Unknown {
                return Err(AppError::conflict(
                    "EXECUTION_VENUE_ORDER_STATUS_UNKNOWN",
                    format!("venue status is unclear for managed order {}", order.id),
                ));
            }
        }
        Ok(())
    }
}

pub(super) fn is_open_like_order_status(status: ManagedOrderStatus) -> bool {
    matches!(
        status,
        ManagedOrderStatus::Planned
            | ManagedOrderStatus::Submitting
            | ManagedOrderStatus::Open
            | ManagedOrderStatus::PartiallyFilled
            | ManagedOrderStatus::CancelPending
            | ManagedOrderStatus::Unknown
    )
}

fn managed_order_status(status: PolymarketOrderLifecycleStatus) -> ManagedOrderStatus {
    match status {
        PolymarketOrderLifecycleStatus::Open => ManagedOrderStatus::Open,
        PolymarketOrderLifecycleStatus::PartiallyFilled => ManagedOrderStatus::PartiallyFilled,
        PolymarketOrderLifecycleStatus::Filled => ManagedOrderStatus::Filled,
        PolymarketOrderLifecycleStatus::Cancelled => ManagedOrderStatus::Cancelled,
        PolymarketOrderLifecycleStatus::Rejected => ManagedOrderStatus::Rejected,
        PolymarketOrderLifecycleStatus::Expired => ManagedOrderStatus::Expired,
        PolymarketOrderLifecycleStatus::Unknown => ManagedOrderStatus::Unknown,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn maps_every_venue_lifecycle_status() {
        let cases = [
            (
                PolymarketOrderLifecycleStatus::Open,
                ManagedOrderStatus::Open,
            ),
            (
                PolymarketOrderLifecycleStatus::PartiallyFilled,
                ManagedOrderStatus::PartiallyFilled,
            ),
            (
                PolymarketOrderLifecycleStatus::Filled,
                ManagedOrderStatus::Filled,
            ),
            (
                PolymarketOrderLifecycleStatus::Cancelled,
                ManagedOrderStatus::Cancelled,
            ),
            (
                PolymarketOrderLifecycleStatus::Rejected,
                ManagedOrderStatus::Rejected,
            ),
            (
                PolymarketOrderLifecycleStatus::Expired,
                ManagedOrderStatus::Expired,
            ),
            (
                PolymarketOrderLifecycleStatus::Unknown,
                ManagedOrderStatus::Unknown,
            ),
        ];
        for (venue, managed) in cases {
            assert_eq!(managed_order_status(venue), managed);
        }
    }
}
