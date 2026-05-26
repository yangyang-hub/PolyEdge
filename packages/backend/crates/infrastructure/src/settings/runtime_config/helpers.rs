fn push_worker_runtime_config_entries(
    entries: &mut Vec<RuntimeConfigEntry>,
    settings: &Settings,
    defaults: &Settings,
) {
    let worker_entries = [
        (
            "poll_news",
            "Worker poll news",
            "POLYEDGE_WORKER__POLL_NEWS",
            settings.worker.poll_news.to_string(),
            defaults.worker.poll_news.to_string(),
            RuntimeConfigValueType::Boolean,
        ),
        (
            "promote_news_events",
            "Worker promote news events",
            "POLYEDGE_WORKER__PROMOTE_NEWS_EVENTS",
            settings.worker.promote_news_events.to_string(),
            defaults.worker.promote_news_events.to_string(),
            RuntimeConfigValueType::Boolean,
        ),
        (
            "poll_arbitrage_radar",
            "Worker poll arbitrage radar",
            "POLYEDGE_WORKER__POLL_ARBITRAGE_RADAR",
            settings.worker.poll_arbitrage_radar.to_string(),
            defaults.worker.poll_arbitrage_radar.to_string(),
            RuntimeConfigValueType::Boolean,
        ),
        (
            "analyze_arbitrage_opportunities",
            "Worker analyze arbitrage",
            "POLYEDGE_WORKER__ANALYZE_ARBITRAGE_OPPORTUNITIES",
            settings.worker.analyze_arbitrage_opportunities.to_string(),
            defaults.worker.analyze_arbitrage_opportunities.to_string(),
            RuntimeConfigValueType::Boolean,
        ),
        (
            "poll_reward_bot",
            "Worker poll reward bot",
            "POLYEDGE_WORKER__POLL_REWARD_BOT",
            settings.worker.poll_reward_bot.to_string(),
            defaults.worker.poll_reward_bot.to_string(),
            RuntimeConfigValueType::Boolean,
        ),
        (
            "drain_execution_queue",
            "Worker drain execution queue",
            "POLYEDGE_WORKER__DRAIN_EXECUTION_QUEUE",
            settings.worker.drain_execution_queue.to_string(),
            defaults.worker.drain_execution_queue.to_string(),
            RuntimeConfigValueType::Boolean,
        ),
        (
            "poll_paper_order_statuses",
            "Worker poll paper order statuses",
            "POLYEDGE_WORKER__POLL_PAPER_ORDER_STATUSES",
            settings.worker.poll_paper_order_statuses.to_string(),
            defaults.worker.poll_paper_order_statuses.to_string(),
            RuntimeConfigValueType::Boolean,
        ),
        (
            "reconcile_paper_fills",
            "Worker reconcile paper fills",
            "POLYEDGE_WORKER__RECONCILE_PAPER_FILLS",
            settings.worker.reconcile_paper_fills.to_string(),
            defaults.worker.reconcile_paper_fills.to_string(),
            RuntimeConfigValueType::Boolean,
        ),
        (
            "poll_polymarket_order_statuses",
            "Worker poll Polymarket order statuses",
            "POLYEDGE_WORKER__POLL_POLYMARKET_ORDER_STATUSES",
            settings.worker.poll_polymarket_order_statuses.to_string(),
            defaults.worker.poll_polymarket_order_statuses.to_string(),
            RuntimeConfigValueType::Boolean,
        ),
        (
            "reconcile_polymarket_fills",
            "Worker reconcile Polymarket fills",
            "POLYEDGE_WORKER__RECONCILE_POLYMARKET_FILLS",
            settings.worker.reconcile_polymarket_fills.to_string(),
            defaults.worker.reconcile_polymarket_fills.to_string(),
            RuntimeConfigValueType::Boolean,
        ),
        (
            "consume_polymarket_user_events",
            "Worker consume Polymarket user events",
            "POLYEDGE_WORKER__CONSUME_POLYMARKET_USER_EVENTS",
            settings.worker.consume_polymarket_user_events.to_string(),
            defaults.worker.consume_polymarket_user_events.to_string(),
            RuntimeConfigValueType::Boolean,
        ),
        (
            "news_promotion_interval_secs",
            "News promotion interval seconds",
            "POLYEDGE_WORKER__NEWS_PROMOTION_INTERVAL_SECS",
            settings.worker.news_promotion_interval_secs.to_string(),
            defaults.worker.news_promotion_interval_secs.to_string(),
            RuntimeConfigValueType::Integer,
        ),
        (
            "arbitrage_analysis_interval_secs",
            "Arbitrage analysis interval seconds",
            "POLYEDGE_WORKER__ARBITRAGE_ANALYSIS_INTERVAL_SECS",
            settings.worker.arbitrage_analysis_interval_secs.to_string(),
            defaults.worker.arbitrage_analysis_interval_secs.to_string(),
            RuntimeConfigValueType::Integer,
        ),
        (
            "execution_drain_interval_secs",
            "Execution drain interval seconds",
            "POLYEDGE_WORKER__EXECUTION_DRAIN_INTERVAL_SECS",
            settings.worker.execution_drain_interval_secs.to_string(),
            defaults.worker.execution_drain_interval_secs.to_string(),
            RuntimeConfigValueType::Integer,
        ),
        (
            "order_status_poll_interval_secs",
            "Order status poll interval seconds",
            "POLYEDGE_WORKER__ORDER_STATUS_POLL_INTERVAL_SECS",
            settings.worker.order_status_poll_interval_secs.to_string(),
            defaults.worker.order_status_poll_interval_secs.to_string(),
            RuntimeConfigValueType::Integer,
        ),
        (
            "fill_reconciliation_interval_secs",
            "Fill reconciliation interval seconds",
            "POLYEDGE_WORKER__FILL_RECONCILIATION_INTERVAL_SECS",
            settings
                .worker
                .fill_reconciliation_interval_secs
                .to_string(),
            defaults
                .worker
                .fill_reconciliation_interval_secs
                .to_string(),
            RuntimeConfigValueType::Integer,
        ),
        (
            "polymarket_user_event_restart_interval_secs",
            "Polymarket user event restart interval seconds",
            "POLYEDGE_WORKER__POLYMARKET_USER_EVENT_RESTART_INTERVAL_SECS",
            settings
                .worker
                .polymarket_user_event_restart_interval_secs
                .to_string(),
            defaults
                .worker
                .polymarket_user_event_restart_interval_secs
                .to_string(),
            RuntimeConfigValueType::Integer,
        ),
        (
            "task_limit",
            "Worker task limit",
            "POLYEDGE_WORKER__TASK_LIMIT",
            settings.worker.task_limit.to_string(),
            defaults.worker.task_limit.to_string(),
            RuntimeConfigValueType::Integer,
        ),
    ];

    for (field, label, env_name, value, default_value, value_type) in worker_entries {
        push_runtime_config_entry(
            entries,
            RuntimeConfigEntryDraft {
                section: "worker",
                field,
                label,
                env_name,
                value,
                default_value,
                value_type,
                options: Vec::new(),
            },
        );
    }
}

struct RuntimeConfigEntryDraft<'a> {
    section: &'a str,
    field: &'a str,
    label: &'a str,
    env_name: &'a str,
    value: String,
    default_value: String,
    value_type: RuntimeConfigValueType,
    options: Vec<String>,
}

fn push_runtime_config_entry(
    entries: &mut Vec<RuntimeConfigEntry>,
    draft: RuntimeConfigEntryDraft<'_>,
) {
    entries.push(RuntimeConfigEntry {
        key: format!("{}.{}", draft.section, draft.field),
        section: draft.section.to_string(),
        field: draft.field.to_string(),
        label: draft.label.to_string(),
        env_name: draft.env_name.to_string(),
        value: draft.value,
        default_value: draft.default_value,
        value_type: draft.value_type,
        options: draft.options,
        restart_required: true,
    });
}

fn optional_config_string(value: Option<&str>) -> String {
    value.unwrap_or_default().trim().to_string()
}

fn optional_runtime_config(value: &str) -> Option<String> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        None
    } else {
        Some(trimmed.to_string())
    }
}

fn news_sources_json(sources: &[NewsSourceSettings]) -> String {
    serde_json::to_string(sources)
        .expect("static news source settings must serialize to runtime config JSON")
}

fn parse_bool_runtime_config(key: &str, value: &str) -> polyedge_domain::Result<bool> {
    match value {
        "true" | "1" => Ok(true),
        "false" | "0" => Ok(false),
        _ => Err(AppError::invalid_input(
            "CONFIG_RUNTIME_BOOL_INVALID",
            format!("runtime config key {key} must be a boolean"),
        )),
    }
}

fn parse_u16_runtime_config(key: &str, value: &str) -> polyedge_domain::Result<u16> {
    value.parse::<u16>().map_err(|error| {
        AppError::invalid_input(
            "CONFIG_RUNTIME_U16_INVALID",
            format!("runtime config key {key} must be a u16: {error}"),
        )
    })
}

fn parse_u32_runtime_config(key: &str, value: &str) -> polyedge_domain::Result<u32> {
    value.parse::<u32>().map_err(|error| {
        AppError::invalid_input(
            "CONFIG_RUNTIME_U32_INVALID",
            format!("runtime config key {key} must be a u32: {error}"),
        )
    })
}

fn parse_u64_runtime_config(key: &str, value: &str) -> polyedge_domain::Result<u64> {
    value.parse::<u64>().map_err(|error| {
        AppError::invalid_input(
            "CONFIG_RUNTIME_U64_INVALID",
            format!("runtime config key {key} must be a u64: {error}"),
        )
    })
}

fn parse_usize_runtime_config(key: &str, value: &str) -> polyedge_domain::Result<usize> {
    value.parse::<usize>().map_err(|error| {
        AppError::invalid_input(
            "CONFIG_RUNTIME_USIZE_INVALID",
            format!("runtime config key {key} must be a usize: {error}"),
        )
    })
}

fn parse_decimal_runtime_config(
    key: &str,
    value: &str,
) -> polyedge_domain::Result<rust_decimal::Decimal> {
    rust_decimal::Decimal::from_str_exact(value).map_err(|error| {
        AppError::invalid_input(
            "CONFIG_RUNTIME_DECIMAL_INVALID",
            format!("runtime config key {key} must be a decimal: {error}"),
        )
    })
}

fn parse_probability_runtime_config(
    key: &str,
    value: &str,
) -> polyedge_domain::Result<Probability> {
    Probability::new(parse_decimal_runtime_config(key, value)?)
}

fn parse_edge_runtime_config(key: &str, value: &str) -> polyedge_domain::Result<Edge> {
    Edge::new(parse_decimal_runtime_config(key, value)?)
}

fn parse_quantity_runtime_config(key: &str, value: &str) -> polyedge_domain::Result<Quantity> {
    Quantity::new(parse_decimal_runtime_config(key, value)?)
}

fn parse_exposure_ratio_runtime_config(
    key: &str,
    value: &str,
) -> polyedge_domain::Result<ExposureRatio> {
    ExposureRatio::new(parse_decimal_runtime_config(key, value)?)
}

fn parse_usd_amount_runtime_config(key: &str, value: &str) -> polyedge_domain::Result<UsdAmount> {
    UsdAmount::new(parse_decimal_runtime_config(key, value)?)
}
