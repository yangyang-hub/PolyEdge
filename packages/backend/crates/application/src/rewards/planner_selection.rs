fn selected_reward_quote_mode(
    config: &RewardBotConfig,
    metrics: Option<&RewardMarketBookMetrics>,
) -> RewardPlanQuoteMode {
    if config.quote_mode == RewardQuoteMode::Auto
        && config.selection_mode == RewardSelectionMode::Enforce
    {
        return metrics
            .map(|metrics| metrics.recommended_quote_mode)
            .unwrap_or(RewardPlanQuoteMode::None);
    }
    RewardPlanQuoteMode::Double
}

fn selected_reward_quote_mode_for_planning(
    config: &RewardBotConfig,
    yes_probability: Decimal,
) -> RewardPlanQuoteMode {
    if config.quote_mode != RewardQuoteMode::Auto
        || config.selection_mode != RewardSelectionMode::Enforce
        || !config.dominant_single_side_enabled
    {
        return RewardPlanQuoteMode::Double;
    }

    let no_probability = Decimal::ONE - yes_probability;
    if yes_probability > config.dominant_max_probability
        || no_probability > config.dominant_max_probability
    {
        return RewardPlanQuoteMode::None;
    }
    if yes_probability >= config.dominant_min_probability {
        return RewardPlanQuoteMode::SingleYes;
    }
    if no_probability >= config.dominant_min_probability {
        return RewardPlanQuoteMode::SingleNo;
    }
    RewardPlanQuoteMode::Double
}

fn build_market_book_metrics(
    yes_token: &RewardToken,
    no_token: &RewardToken,
    books: &HashMap<String, RewardOrderBook>,
    yes_probability: Decimal,
    config: &RewardBotConfig,
) -> Option<RewardMarketBookMetrics> {
    let yes = books
        .get(&yes_token.token_id)
        .and_then(reward_book_side_metrics);
    let no = books
        .get(&no_token.token_id)
        .and_then(reward_book_side_metrics);
    let (recommended_quote_mode, reason) =
        recommend_reward_quote_mode(yes_probability, yes.as_ref(), no.as_ref(), config);

    Some(RewardMarketBookMetrics {
        yes_probability: yes_probability.round_dp(6),
        recommended_quote_mode,
        reason,
        yes,
        no,
    })
}

fn recommend_reward_quote_mode(
    yes_probability: Decimal,
    yes: Option<&RewardBookSideMetrics>,
    no: Option<&RewardBookSideMetrics>,
    config: &RewardBotConfig,
) -> (RewardPlanQuoteMode, Option<String>) {
    if !config.dominant_single_side_enabled {
        return (RewardPlanQuoteMode::Double, None);
    }

    let no_probability = Decimal::ONE - yes_probability;
    if yes_probability > config.dominant_max_probability
        || no_probability > config.dominant_max_probability
    {
        return (
            RewardPlanQuoteMode::None,
            Some("dominant probability is beyond configured single-side cap".to_string()),
        );
    }

    if yes_probability >= config.dominant_min_probability {
        return recommend_dominant_side(RewardPlanQuoteMode::SingleYes, "YES", yes, config);
    }
    if no_probability >= config.dominant_min_probability {
        return recommend_dominant_side(RewardPlanQuoteMode::SingleNo, "NO", no, config);
    }

    if let Some(reason) = book_concentration_rejection("YES", yes, config)
        .or_else(|| book_concentration_rejection("NO", no, config))
    {
        return (RewardPlanQuoteMode::None, Some(reason));
    }
    (RewardPlanQuoteMode::Double, None)
}

fn recommend_dominant_side(
    quote_mode: RewardPlanQuoteMode,
    label: &str,
    metrics: Option<&RewardBookSideMetrics>,
    config: &RewardBotConfig,
) -> (RewardPlanQuoteMode, Option<String>) {
    let Some(metrics) = metrics else {
        return (
            RewardPlanQuoteMode::None,
            Some(format!(
                "{label} book metrics unavailable for dominant single-side quote"
            )),
        );
    };
    if let Some(reason) = book_concentration_rejection(label, Some(metrics), config) {
        return (RewardPlanQuoteMode::None, Some(reason));
    }
    if config.dominant_min_exit_depth_usd > Decimal::ZERO
        && metrics.exit_depth_usd < config.dominant_min_exit_depth_usd
    {
        return (
            RewardPlanQuoteMode::None,
            Some(format!(
                "{label} exit depth ${} is below dominant-side minimum ${}",
                metrics.exit_depth_usd, config.dominant_min_exit_depth_usd
            )),
        );
    }
    (quote_mode, None)
}

fn book_concentration_rejection(
    label: &str,
    metrics: Option<&RewardBookSideMetrics>,
    config: &RewardBotConfig,
) -> Option<String> {
    let thresholds_enabled = config.max_top1_depth_share < Decimal::ONE
        || config.max_top3_depth_share < Decimal::ONE
        || config.max_book_hhi < Decimal::ONE;
    if !thresholds_enabled {
        return None;
    }
    let metrics = metrics?;
    if metrics.top1_depth_share > config.max_top1_depth_share {
        return Some(format!(
            "{label} top-1 depth share {} exceeds {}",
            metrics.top1_depth_share, config.max_top1_depth_share
        ));
    }
    if metrics.top3_depth_share > config.max_top3_depth_share {
        return Some(format!(
            "{label} top-3 depth share {} exceeds {}",
            metrics.top3_depth_share, config.max_top3_depth_share
        ));
    }
    if metrics.book_hhi > config.max_book_hhi {
        return Some(format!(
            "{label} book HHI {} exceeds {}",
            metrics.book_hhi, config.max_book_hhi
        ));
    }
    None
}

fn reward_book_side_metrics(book: &RewardOrderBook) -> Option<RewardBookSideMetrics> {
    let notionals = book
        .bids
        .iter()
        .filter(|level| level.price > Decimal::ZERO && level.size > Decimal::ZERO)
        .take(10)
        .map(|level| (level.price * level.size).round_dp(4))
        .filter(|notional| *notional > Decimal::ZERO)
        .collect::<Vec<_>>();
    let total: Decimal = notionals.iter().copied().sum();
    if total <= Decimal::ZERO {
        return None;
    }
    let top1: Decimal = notionals.iter().take(1).copied().sum();
    let top3: Decimal = notionals.iter().take(3).copied().sum();
    let book_hhi = notionals
        .iter()
        .map(|notional| {
            let share = *notional / total;
            share * share
        })
        .sum::<Decimal>()
        .round_dp(6);

    Some(RewardBookSideMetrics {
        top1_depth_share: (top1 / total).round_dp(6),
        top3_depth_share: (top3 / total).round_dp(6),
        book_hhi,
        exit_depth_usd: total.round_dp(4),
    })
}

fn bid_touches_ask(state: &Option<TokenBookState>, bid: Decimal) -> bool {
    state
        .as_ref()
        .and_then(|state| state.best_ask)
        .is_some_and(|best_ask| bid >= best_ask)
}

fn make_single_quote_leg(
    token: &RewardToken,
    price: Decimal,
    rewards_min_size: Decimal,
) -> Option<RewardQuoteLeg> {
    let minimum_size = minimum_live_quote_size(price, rewards_min_size);
    let notional = price * minimum_size;
    let leg = make_leg(token, price, notional);
    if rewards_min_size > Decimal::ZERO && leg.size < rewards_min_size {
        return None;
    }
    Some(leg)
}

fn preferred_category_bonus(market: &RewardMarket, config: &RewardBotConfig) -> Decimal {
    if config.preferred_category_score_bonus <= Decimal::ZERO {
        return Decimal::ZERO;
    }
    let category = market.category.trim().to_ascii_lowercase();
    if category.is_empty() || !config.preferred_categories.contains(&category) {
        return Decimal::ZERO;
    }
    config.preferred_category_score_bonus
}
