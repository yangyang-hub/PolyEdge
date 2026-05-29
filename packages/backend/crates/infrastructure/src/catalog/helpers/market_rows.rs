fn parse_market_row(row: &sqlx::postgres::PgRow) -> Result<MarketView> {
    let status_raw: String = decode_column(row, "status")?;
    let ambiguity_level_raw: String = decode_column(row, "ambiguity_level")?;
    let tradability_status_raw: String = decode_column(row, "tradability_status")?;
    let best_bid: Decimal = decode_column(row, "best_bid")?;
    let best_ask: Decimal = decode_column(row, "best_ask")?;
    let mid_price: Decimal = decode_column(row, "mid_price")?;
    let volume_24h: Decimal = decode_column(row, "volume_24h")?;

    Ok(MarketView {
        id: decode_column(row, "id")?,
        slug: decode_column(row, "slug")?,
        question: decode_column(row, "question")?,
        category: decode_column(row, "category")?,
        status: MarketStatus::from_str(&status_raw).map_err(|error| {
            db_error(
                "POSTGRES_DECODE_FAILED",
                format!("failed to decode market status: {error}"),
            )
        })?,
        best_bid: Probability::new(best_bid).map_err(|error| {
            db_error(
                "POSTGRES_DECODE_FAILED",
                format!("failed to decode market best_bid: {error}"),
            )
        })?,
        best_ask: Probability::new(best_ask).map_err(|error| {
            db_error(
                "POSTGRES_DECODE_FAILED",
                format!("failed to decode market best_ask: {error}"),
            )
        })?,
        mid_price: Probability::new(mid_price).map_err(|error| {
            db_error(
                "POSTGRES_DECODE_FAILED",
                format!("failed to decode market mid_price: {error}"),
            )
        })?,
        volume_24h: UsdAmount::new(volume_24h).map_err(|error| {
            db_error(
                "POSTGRES_DECODE_FAILED",
                format!("failed to decode market volume_24h: {error}"),
            )
        })?,
        ambiguity_level: AmbiguityLevel::from_str(&ambiguity_level_raw).map_err(|error| {
            db_error(
                "POSTGRES_DECODE_FAILED",
                format!("failed to decode ambiguity level: {error}"),
            )
        })?,
        tradability_status: TradabilityStatus::from_str(&tradability_status_raw).map_err(
            |error| {
                db_error(
                    "POSTGRES_DECODE_FAILED",
                    format!("failed to decode tradability status: {error}"),
                )
            },
        )?,
        resolution_source: decode_column(row, "resolution_source")?,
        edge_case_notes: decode_column(row, "edge_case_notes")?,
        polymarket_condition_id: decode_column(row, "polymarket_condition_id")?,
        polymarket_yes_asset_id: decode_column(row, "polymarket_yes_asset_id")?,
        polymarket_no_asset_id: decode_column(row, "polymarket_no_asset_id")?,
        updated_at: decode_column(row, "updated_at")?,
        version: decode_column(row, "version")?,
    })
}
