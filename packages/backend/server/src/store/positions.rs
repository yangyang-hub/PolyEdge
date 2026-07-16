use super::*;
use crate::execution::{
    WalletPositionRiskTotals, WalletPositionSnapshot, WalletPositionSnapshotEntry,
};

struct KnownOutcome {
    market_id: i64,
    outcome: String,
    token_id: String,
}

pub(super) async fn replace_wallet_positions(
    store: &PostgresStore,
    wallet_id: i64,
    market_id: i64,
    snapshot: WalletPositionSnapshot,
) -> Result<WalletPositionRiskTotals> {
    validate_snapshot(&snapshot)?;
    let mut tx = store.pool.begin().await?;
    let owner_user_id: i64 = sqlx::query_scalar(
        "SELECT w.owner_user_id FROM wallet_account_state a JOIN wallet_accounts w ON w.wallet_id = a.wallet_id WHERE a.wallet_id = $1 FOR UPDATE OF a",
    )
        .bind(wallet_id)
        .fetch_optional(&mut *tx)
        .await?
        .ok_or_else(|| ServerError::NotFound("wallet account state not found".to_string()))?;

    let known = sqlx::query(
        r#"
        SELECT market_id, outcome, token_id
        FROM managed_market_outcomes
        ORDER BY outcome_id
        "#,
    )
    .fetch_all(&mut *tx)
    .await?
    .into_iter()
    .map(|row| {
        Ok(KnownOutcome {
            market_id: row.try_get("market_id")?,
            outcome: row.try_get("outcome")?,
            token_id: row.try_get("token_id")?,
        })
    })
    .collect::<Result<Vec<_>>>()?;
    let existing_realized =
        sqlx::query("SELECT token_id,realized_pnl FROM positions WHERE wallet_id=$1")
            .bind(wallet_id)
            .fetch_all(&mut *tx)
            .await?
            .into_iter()
            .map(|row| {
                Ok((
                    row.try_get::<String, _>("token_id")?,
                    row.try_get::<Decimal, _>("realized_pnl")?,
                ))
            })
            .collect::<Result<HashMap<_, _>>>()?;

    let incoming = snapshot
        .positions
        .into_iter()
        .map(|position| (position.token_id.clone(), position))
        .collect::<HashMap<_, _>>();
    let mut market_ids = Vec::with_capacity(known.len());
    let mut outcomes = Vec::with_capacity(known.len());
    let mut token_ids = Vec::with_capacity(known.len());
    let mut quantities = Vec::with_capacity(known.len());
    let mut average_prices = Vec::with_capacity(known.len());
    let mut realized_pnls = Vec::with_capacity(known.len());
    for outcome in known {
        let position = incoming.get(&outcome.token_id);
        let realized_pnl = position.map_or_else(
            || {
                existing_realized
                    .get(&outcome.token_id)
                    .copied()
                    .unwrap_or(Decimal::ZERO)
            },
            |value| value.realized_pnl,
        );
        market_ids.push(outcome.market_id);
        outcomes.push(outcome.outcome);
        token_ids.push(outcome.token_id);
        quantities.push(position.map_or(Decimal::ZERO, |value| value.quantity));
        average_prices.push(position.map_or(Decimal::ZERO, |value| value.average_price));
        realized_pnls.push(realized_pnl);
    }

    sqlx::query(
        r#"
        INSERT INTO positions (
          owner_user_id, wallet_id, market_id, token_id, outcome, quantity, average_price,
          realized_pnl, version, observed_at
        )
        SELECT $1, $2, snapshot.market_id, snapshot.token_id, snapshot.outcome,
               snapshot.quantity, snapshot.average_price, snapshot.realized_pnl,
               1, $9
        FROM UNNEST(
          $3::BIGINT[], $4::TEXT[], $5::TEXT[], $6::NUMERIC[],
          $7::NUMERIC[], $8::NUMERIC[]
        ) AS snapshot(
          market_id, outcome, token_id, quantity, average_price, realized_pnl
        )
        ON CONFLICT (wallet_id, token_id) DO UPDATE
        SET market_id = EXCLUDED.market_id,
            outcome = EXCLUDED.outcome,
            quantity = EXCLUDED.quantity,
            average_price = EXCLUDED.average_price,
            realized_pnl = EXCLUDED.realized_pnl,
            version = positions.version + 1,
            observed_at = EXCLUDED.observed_at,
            updated_at = now()
        WHERE positions.observed_at <= EXCLUDED.observed_at
        "#,
    )
    .bind(owner_user_id)
    .bind(wallet_id)
    .bind(&market_ids)
    .bind(&outcomes)
    .bind(&token_ids)
    .bind(&quantities)
    .bind(&average_prices)
    .bind(&realized_pnls)
    .bind(snapshot.observed_at)
    .execute(&mut *tx)
    .await?;

    let totals = sqlx::query(
        r#"
        SELECT
          COALESCE(SUM(p.quantity * p.average_price), 0) AS total_notional,
          COALESCE(SUM(p.quantity * p.average_price)
            FILTER (WHERE p.market_id = $2), 0) AS market_notional
        FROM positions p
        JOIN managed_market_outcomes o ON o.token_id = p.token_id
        WHERE p.wallet_id = $1
        "#,
    )
    .bind(wallet_id)
    .bind(market_id)
    .fetch_one(&mut *tx)
    .await?;
    let total_position_notional: Decimal = totals.try_get("total_notional")?;
    let market_position_notional: Decimal = totals.try_get("market_notional")?;
    let updated = sqlx::query(
        r#"
        UPDATE wallet_account_state
        SET total_position_notional = $2, last_synced_at = $3,
            last_error = NULL, version = version + 1, updated_at = now()
        WHERE wallet_id = $1
        "#,
    )
    .bind(wallet_id)
    .bind(total_position_notional)
    .bind(snapshot.observed_at)
    .execute(&mut *tx)
    .await?;
    if updated.rows_affected() != 1 {
        return Err(ServerError::Conflict(
            "wallet account state changed during position reconciliation".to_string(),
        ));
    }
    let equity = sqlx::query(
        r#"SELECT s.available_collateral, s.reserved_collateral,
                  COALESCE(SUM(p.realized_pnl),0) AS realized_pnl
           FROM wallet_account_state s
           LEFT JOIN positions p ON p.wallet_id=s.wallet_id
           WHERE s.wallet_id=$1
           GROUP BY s.available_collateral,s.reserved_collateral"#,
    )
    .bind(wallet_id)
    .fetch_one(&mut *tx)
    .await?;
    let available: Decimal = equity.try_get("available_collateral")?;
    let reserved: Decimal = equity.try_get("reserved_collateral")?;
    let realized_pnl: Decimal = equity.try_get("realized_pnl")?;
    sqlx::query(
        r#"INSERT INTO wallet_equity_snapshots (
             owner_user_id,wallet_id,collateral_balance,position_market_value,
             realized_pnl,unrealized_pnl,total_equity,valuation_status,observed_at
           ) VALUES ($1,$2,$3,$4,$5,0,$6,'partial',$7)"#,
    )
    .bind(owner_user_id)
    .bind(wallet_id)
    .bind(available + reserved)
    .bind(total_position_notional)
    .bind(realized_pnl)
    .bind(available + reserved + total_position_notional)
    .bind(snapshot.observed_at)
    .execute(&mut *tx)
    .await?;
    tx.commit().await?;
    Ok(WalletPositionRiskTotals {
        total_position_notional,
        market_position_notional,
    })
}

fn validate_snapshot(snapshot: &WalletPositionSnapshot) -> Result<()> {
    let mut tokens = std::collections::HashSet::new();
    for position in &snapshot.positions {
        validate_position(position)?;
        if !tokens.insert(position.token_id.as_str()) {
            return Err(ServerError::Conflict(
                "wallet position snapshot contains duplicate token IDs".to_string(),
            ));
        }
    }
    Ok(())
}

fn validate_position(position: &WalletPositionSnapshotEntry) -> Result<()> {
    if position.token_id.trim().is_empty()
        || position.quantity < Decimal::ZERO
        || position.average_price < Decimal::ZERO
        || position.average_price >= Decimal::ONE
    {
        return Err(ServerError::InvalidInput(
            "wallet position snapshot contains invalid values".to_string(),
        ));
    }
    Ok(())
}
