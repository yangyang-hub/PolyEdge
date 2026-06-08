#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct RewardListPage {
    pub page: usize,
    pub page_size: usize,
    pub total_items: usize,
    pub total_pages: usize,
}

impl RewardListPage {
    #[must_use]
    pub fn new(page: usize, page_size: usize, total_items: usize) -> Self {
        let page_size = page_size.clamp(1, usize::from(MAX_LIST_LIMIT));
        let total_pages = if total_items == 0 {
            1
        } else {
            total_items.div_ceil(page_size)
        };
        let page = page.clamp(1, total_pages);

        Self {
            page,
            page_size,
            total_items,
            total_pages,
        }
    }
}

impl Default for RewardListPage {
    fn default() -> Self {
        Self::new(1, usize::from(DEFAULT_LIST_LIMIT), 0)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct RewardOrderPage {
    pub items: Vec<ManagedRewardOrder>,
    pub page: RewardListPage,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RewardOrderStatusFilter {
    Open,
    Filled,
    Cancelled,
    ExitPending,
}

impl RewardOrderStatusFilter {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Open => "open",
            Self::Filled => "filled",
            Self::Cancelled => "cancelled",
            Self::ExitPending => "exit_pending",
        }
    }

    #[must_use]
    pub fn from_optional_str(value: Option<String>) -> Option<Self> {
        match value?.trim().to_lowercase().as_str() {
            "open" => Some(Self::Open),
            "filled" => Some(Self::Filled),
            "cancelled" => Some(Self::Cancelled),
            "exit_pending" => Some(Self::ExitPending),
            _ => None,
        }
    }

    #[must_use]
    fn matches(self, status: ManagedRewardOrderStatus) -> bool {
        match self {
            Self::Open => {
                matches!(
                    status,
                    ManagedRewardOrderStatus::Open | ManagedRewardOrderStatus::Planned
                )
            }
            Self::Filled => status == ManagedRewardOrderStatus::Filled,
            Self::Cancelled => status == ManagedRewardOrderStatus::Cancelled,
            Self::ExitPending => status == ManagedRewardOrderStatus::ExitPending,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RewardOrderSortField {
    Status,
    Price,
    Size,
}

impl RewardOrderSortField {
    #[must_use]
    fn from_optional_str(value: Option<String>) -> Self {
        match value.as_deref().map(str::trim) {
            Some("price") => Self::Price,
            Some("size") => Self::Size,
            _ => Self::Status,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RewardOrderListQuery {
    pub account_id: String,
    pub search: Option<String>,
    pub status: Option<RewardOrderStatusFilter>,
    pub sort_by: RewardOrderSortField,
    pub sort_order: SortOrder,
    pub page: usize,
    pub page_size: usize,
}

impl RewardOrderListQuery {
    #[must_use]
    pub fn new(
        account_id: String,
        search: Option<String>,
        status: Option<String>,
        sort_by: Option<String>,
        sort_order: Option<String>,
        page: Option<u16>,
        page_size: Option<u16>,
    ) -> Self {
        Self {
            account_id,
            search: normalize_order_search(search),
            status: RewardOrderStatusFilter::from_optional_str(status),
            sort_by: RewardOrderSortField::from_optional_str(sort_by),
            sort_order: parse_sort_order(sort_order),
            page: usize::from(page.unwrap_or(1).max(1)),
            page_size: usize::from(validate_reward_list_limit(page_size)),
        }
    }

    #[must_use]
    pub fn page_for_total(&self, total_items: usize) -> RewardListPage {
        RewardListPage::new(self.page, self.page_size, total_items)
    }

    #[must_use]
    pub fn matches_order(&self, order: &ManagedRewardOrder) -> bool {
        if let Some(status) = self.status
            && !status.matches(order.status)
        {
            return false;
        }

        let Some(search) = self.search.as_deref() else {
            return true;
        };
        order.outcome.to_lowercase().contains(search)
            || order.condition_id.to_lowercase().contains(search)
            || order.token_id.to_lowercase().contains(search)
    }

    #[must_use]
    pub fn compare_orders(
        &self,
        left: &ManagedRewardOrder,
        right: &ManagedRewardOrder,
    ) -> std::cmp::Ordering {
        let open_group = order_open_group(left).cmp(&order_open_group(right));
        if open_group != std::cmp::Ordering::Equal {
            return open_group;
        }

        let field_order = match self.sort_by {
            RewardOrderSortField::Price => left.price.cmp(&right.price),
            RewardOrderSortField::Size => left.size.cmp(&right.size),
            RewardOrderSortField::Status => left.status.as_str().cmp(right.status.as_str()),
        };
        let field_order = match self.sort_order {
            SortOrder::Asc => field_order,
            SortOrder::Desc => field_order.reverse(),
        };

        field_order.then_with(|| right.updated_at.cmp(&left.updated_at))
    }
}

impl Default for RewardOrderListQuery {
    fn default() -> Self {
        Self::new(
            String::new(),
            None,
            None,
            None,
            None,
            Some(1),
            Some(DEFAULT_LIST_LIMIT),
        )
    }
}

fn normalize_order_search(search: Option<String>) -> Option<String> {
    let search = search?.trim().to_lowercase();
    if search.is_empty() {
        None
    } else {
        Some(search)
    }
}

// ---------------------------------------------------------------------------
// Quote-plan pagination
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RewardQuotePlanSortField {
    Score,
    DailyReward,
    Midpoint,
    Eligible,
}

impl RewardQuotePlanSortField {
    #[must_use]
    pub fn from_optional_str(value: Option<String>) -> Self {
        match value.as_deref().map(str::trim) {
            Some("daily_reward") => Self::DailyReward,
            Some("midpoint") => Self::Midpoint,
            Some("eligible") => Self::Eligible,
            _ => Self::Score,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RewardQuotePlanListQuery {
    pub search: Option<String>,
    pub eligible: Option<bool>,
    pub sort_by: RewardQuotePlanSortField,
    pub sort_order: SortOrder,
    pub page: usize,
    pub page_size: usize,
}

impl RewardQuotePlanListQuery {
    #[must_use]
    pub fn new(
        search: Option<String>,
        eligible: Option<bool>,
        sort_by: Option<String>,
        sort_order: Option<String>,
        page: Option<u16>,
        page_size: Option<u16>,
    ) -> Self {
        Self {
            search: normalize_plan_search(search),
            eligible,
            sort_by: RewardQuotePlanSortField::from_optional_str(sort_by),
            sort_order: parse_sort_order(sort_order),
            page: usize::from(page.unwrap_or(1).max(1)),
            page_size: usize::from(validate_reward_list_limit(page_size)),
        }
    }

    #[must_use]
    pub fn page_for_total(&self, total_items: usize) -> RewardListPage {
        RewardListPage::new(self.page, self.page_size, total_items)
    }
}

impl Default for RewardQuotePlanListQuery {
    fn default() -> Self {
        Self::new(None, None, None, None, Some(1), Some(DEFAULT_LIST_LIMIT))
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct RewardQuotePlanPage {
    pub items: Vec<RewardQuotePlan>,
    pub page: RewardListPage,
}

fn normalize_plan_search(search: Option<String>) -> Option<String> {
    let search = search?.trim().to_lowercase();
    if search.is_empty() {
        None
    } else {
        Some(search)
    }
}

fn parse_sort_order(sort_order: Option<String>) -> SortOrder {
    match sort_order.as_deref().map(str::trim) {
        Some("asc") => SortOrder::Asc,
        _ => SortOrder::Desc,
    }
}

fn order_open_group(order: &ManagedRewardOrder) -> u8 {
    if order.status.is_open_like() { 0 } else { 1 }
}
