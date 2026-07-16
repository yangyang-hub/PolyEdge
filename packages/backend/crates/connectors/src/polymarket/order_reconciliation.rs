fn order_lifecycle_status(
    order: &OpenOrderResponse,
    now: OffsetDateTime,
) -> PolymarketOrderLifecycleStatus {
    classify_order_lifecycle(
        &order.status,
        order.original_size,
        order.size_matched,
        order.expiration.timestamp(),
        now.unix_timestamp(),
    )
}

fn classify_order_lifecycle(
    status: &SdkOrderStatusType,
    original_size: Decimal,
    size_matched: Decimal,
    expiration_timestamp: i64,
    now_timestamp: i64,
) -> PolymarketOrderLifecycleStatus {
    use PolymarketOrderLifecycleStatus as Lifecycle;

    let filled = size_matched.max(Decimal::ZERO);
    match status {
        SdkOrderStatusType::Canceled => Lifecycle::Cancelled,
        SdkOrderStatusType::Matched if original_size > Decimal::ZERO => {
            if filled >= original_size {
                Lifecycle::Filled
            } else if filled > Decimal::ZERO {
                Lifecycle::PartiallyFilled
            } else {
                Lifecycle::Unknown
            }
        }
        SdkOrderStatusType::Live
        | SdkOrderStatusType::Unmatched
        | SdkOrderStatusType::Delayed => {
            if expiration_timestamp > 0 && expiration_timestamp <= now_timestamp {
                Lifecycle::Expired
            } else if filled > Decimal::ZERO {
                Lifecycle::PartiallyFilled
            } else {
                Lifecycle::Open
            }
        }
        SdkOrderStatusType::Unknown(raw) => match raw.trim().to_ascii_uppercase().as_str() {
            "CANCELED" | "CANCELLED" => Lifecycle::Cancelled,
            "REJECTED" => Lifecycle::Rejected,
            "EXPIRED" => Lifecycle::Expired,
            "FILLED" => Lifecycle::Filled,
            "PARTIALLY_FILLED" | "PARTIAL" => Lifecycle::PartiallyFilled,
            "LIVE" | "OPEN" | "UNMATCHED" | "DELAYED" => Lifecycle::Open,
            _ => Lifecycle::Unknown,
        },
        _ => Lifecycle::Unknown,
    }
}

#[cfg(test)]
mod order_reconciliation_tests {
    use super::*;

    #[test]
    fn maps_live_fill_progress_and_expiration() {
        assert_eq!(
            classify_order_lifecycle(
                &SdkOrderStatusType::Live,
                Decimal::TEN,
                Decimal::ZERO,
                200,
                100,
            ),
            PolymarketOrderLifecycleStatus::Open
        );
        assert_eq!(
            classify_order_lifecycle(
                &SdkOrderStatusType::Live,
                Decimal::TEN,
                Decimal::ONE,
                200,
                100,
            ),
            PolymarketOrderLifecycleStatus::PartiallyFilled
        );
        assert_eq!(
            classify_order_lifecycle(
                &SdkOrderStatusType::Live,
                Decimal::TEN,
                Decimal::ZERO,
                100,
                100,
            ),
            PolymarketOrderLifecycleStatus::Expired
        );
    }

    #[test]
    fn maps_terminal_and_raw_statuses() {
        assert_eq!(
            classify_order_lifecycle(
                &SdkOrderStatusType::Matched,
                Decimal::TEN,
                Decimal::TEN,
                0,
                100,
            ),
            PolymarketOrderLifecycleStatus::Filled
        );
        assert_eq!(
            classify_order_lifecycle(
                &SdkOrderStatusType::Canceled,
                Decimal::TEN,
                Decimal::ONE,
                0,
                100,
            ),
            PolymarketOrderLifecycleStatus::Cancelled
        );
        assert_eq!(
            classify_order_lifecycle(
                &SdkOrderStatusType::Unknown("REJECTED".to_string()),
                Decimal::TEN,
                Decimal::ZERO,
                0,
                100,
            ),
            PolymarketOrderLifecycleStatus::Rejected
        );
        assert_eq!(
            classify_order_lifecycle(
                &SdkOrderStatusType::Unknown("future_status".to_string()),
                Decimal::TEN,
                Decimal::ZERO,
                0,
                100,
            ),
            PolymarketOrderLifecycleStatus::Unknown
        );
    }
}
