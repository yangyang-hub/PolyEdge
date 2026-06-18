#[derive(Debug, Clone)]
struct ConsoleRiskSnapshot {
    risk_state: RiskStateView,
    environment: String,
    alerts: Vec<RiskAlertData>,
    buckets: Vec<RiskBucketData>,
}

#[derive(Debug, Clone)]
struct BucketAccumulator {
    exposure: Decimal,
    updated_at: OffsetDateTime,
    version: i64,
}

fn daily_loss_used(risk_state: &RiskStateView) -> polyedge_domain::Result<UsdAmount> {
    let daily_pnl = risk_state.daily_pnl.value();

    if daily_pnl < Decimal::ZERO {
        return UsdAmount::new(-daily_pnl);
    }

    UsdAmount::new(Decimal::ZERO)
}

async fn read_console_risk_snapshot(
    state: &AppState,
) -> polyedge_domain::Result<ConsoleRiskSnapshot> {
    let risk_state = state.risk_service.read_state().await?;
    let mode = state.system_mode_service.read_mode().await?;
    let positions = state
        .execution_service
        .list_positions(PositionListFilters {
            market_id: None,
            connector_name: None,
            side: None,
            limit: u16::MAX,
        })
        .await?;
    let market_ids = positions
        .iter()
        .map(|position| position.market_id.clone())
        .collect::<Vec<_>>();
    let markets = state
        .market_event_service
        .get_markets_by_ids(&market_ids)
        .await?;
    let markets_by_id = markets
        .iter()
        .map(|market| (market.id.clone(), market.clone()))
        .collect::<HashMap<_, _>>();
    let buckets = derive_risk_buckets(&positions, &markets_by_id)?;
    let alerts = derive_risk_alerts(&risk_state, &buckets, state.risk_service.policy())?;

    Ok(ConsoleRiskSnapshot {
        risk_state,
        environment: mode.environment,
        alerts,
        buckets,
    })
}

fn derive_risk_buckets(
    positions: &[PositionView],
    markets_by_id: &HashMap<String, MarketView>,
) -> polyedge_domain::Result<Vec<RiskBucketData>> {
    let mut grouped = HashMap::<String, BucketAccumulator>::new();

    for position in positions {
        let bucket_name = markets_by_id
            .get(&position.market_id)
            .map(|market| market.category.clone())
            .unwrap_or_else(|| "Uncategorized".to_string());
        let exposure = (position.net_quantity.value() * position.mark_price.value()).abs();

        grouped
            .entry(bucket_name)
            .and_modify(|bucket| {
                bucket.exposure += exposure;
                bucket.updated_at = bucket.updated_at.max(position.updated_at);
                bucket.version = bucket.version.max(position.version);
            })
            .or_insert(BucketAccumulator {
                exposure,
                updated_at: position.updated_at,
                version: position.version,
            });
    }

    let total_exposure = grouped
        .values()
        .fold(Decimal::ZERO, |sum, bucket| sum + bucket.exposure);
    let mut buckets = grouped
        .into_iter()
        .map(|(name, bucket)| {
            let exposure_ratio = if total_exposure > Decimal::ZERO {
                bucket.exposure / total_exposure
            } else {
                Decimal::ZERO
            };
            let limit = category_limit(&name)?;
            let utilization = if limit.value() > Decimal::ZERO {
                exposure_ratio / limit.value()
            } else {
                Decimal::ZERO
            };
            let status = if utilization >= Decimal::ONE {
                BucketStatus::Breach
            } else if utilization >= Decimal::new(85, 2) {
                BucketStatus::Watch
            } else {
                BucketStatus::Healthy
            };

            Ok(RiskBucketData {
                id: format!("bucket_{}", slugify(&name)),
                name,
                exposure: ExposureRatio::new(exposure_ratio)?,
                limit,
                utilization: ExposureRatio::new(utilization)?,
                status,
                updated_at: bucket.updated_at,
                version: bucket.version,
            })
        })
        .collect::<polyedge_domain::Result<Vec<_>>>()?;

    buckets.sort_by(|left, right| right.exposure.cmp(&left.exposure));
    Ok(buckets)
}

fn derive_risk_alerts(
    risk_state: &RiskStateView,
    buckets: &[RiskBucketData],
    policy: &RiskPolicy,
) -> polyedge_domain::Result<Vec<RiskAlertData>> {
    let mut alerts = Vec::new();
    let daily_loss_used = daily_loss_used(risk_state)?;
    let daily_loss_limit = policy.max_daily_loss.value();
    let daily_loss_usage = if daily_loss_limit > Decimal::ZERO {
        daily_loss_used.value() / daily_loss_limit
    } else {
        Decimal::ZERO
    };

    if risk_state.kill_switch {
        alerts.push(RiskAlertData {
            id: "alt_kill_switch_active".to_string(),
            severity: AlertSeverity::Critical,
            reason: "Kill switch is active. Execution remains halted until a protected release completes.".to_string(),
            target: "System Runtime".to_string(),
            status: AlertStatus::Unresolved,
            created_at: risk_state.updated_at,
            updated_at: risk_state.updated_at,
            version: risk_state.version,
        });
    }

    if daily_loss_usage >= Decimal::new(8, 1) {
        alerts.push(RiskAlertData {
            id: "alt_daily_loss_usage".to_string(),
            severity: if daily_loss_usage >= Decimal::new(9, 1) {
                AlertSeverity::Critical
            } else {
                AlertSeverity::Warning
            },
            reason: format!(
                "Daily loss usage reached {}% of the configured budget.",
                (daily_loss_usage * Decimal::new(100, 0)).round_dp(0)
            ),
            target: "Global Risk".to_string(),
            status: if daily_loss_usage >= Decimal::new(9, 1) {
                AlertStatus::Unresolved
            } else {
                AlertStatus::Watching
            },
            created_at: risk_state.updated_at,
            updated_at: risk_state.updated_at,
            version: risk_state.version,
        });
    }

    for bucket in buckets {
        if bucket.status == BucketStatus::Healthy {
            continue;
        }

        alerts.push(RiskAlertData {
            id: format!("alt_bucket_{}", bucket.id),
            severity: if bucket.status == BucketStatus::Breach {
                AlertSeverity::Critical
            } else {
                AlertSeverity::Warning
            },
            reason: if bucket.status == BucketStatus::Breach {
                format!(
                    "{} exposure exceeded its configured concentration limit.",
                    bucket.name
                )
            } else {
                format!(
                    "{} exposure is approaching its configured concentration limit.",
                    bucket.name
                )
            },
            target: format!("{} Bucket", bucket.name),
            status: if bucket.status == BucketStatus::Breach {
                AlertStatus::Unresolved
            } else {
                AlertStatus::Watching
            },
            created_at: bucket.updated_at,
            updated_at: bucket.updated_at,
            version: bucket.version,
        });
    }

    alerts.sort_by(|left, right| {
        alert_severity_rank(left.severity)
            .cmp(&alert_severity_rank(right.severity))
            .then_with(|| right.updated_at.cmp(&left.updated_at))
    });
    Ok(alerts)
}

fn alert_severity_rank(severity: AlertSeverity) -> u8 {
    match severity {
        AlertSeverity::Critical => 0,
        AlertSeverity::Warning => 1,
    }
}

fn category_limit(category: &str) -> polyedge_domain::Result<ExposureRatio> {
    let limit = match category.to_lowercase().as_str() {
        "crypto" => Decimal::new(35, 2),
        "regulation" => Decimal::new(25, 2),
        "macro" => Decimal::new(18, 2),
        _ => Decimal::new(20, 2),
    };

    ExposureRatio::new(limit)
}

fn slugify(value: &str) -> String {
    let slug = value
        .chars()
        .map(|character| {
            if character.is_ascii_alphanumeric() {
                character.to_ascii_lowercase()
            } else {
                '_'
            }
        })
        .collect::<String>()
        .trim_matches('_')
        .to_string();

    if slug.is_empty() {
        "uncategorized".to_string()
    } else {
        slug
    }
}
