use serde::{Deserialize, Serialize};

use crate::market_event::SortOrder;

/// Default page size for paginated list endpoints.
pub const DEFAULT_PAGE_SIZE: u16 = 20;

/// Maximum page size allowed across all paginated endpoints.
pub const MAX_PAGE_SIZE: u16 = 200;

/// Query parameters for paginated list endpoints.
///
/// Parsed from the API query string by each handler.
#[derive(Debug, Clone, Deserialize)]
pub struct PageQuery {
    /// 1-based page number. Clamped to >= 1.
    #[serde(default = "default_page")]
    pub page: u32,
    /// Number of items per page. Clamped to [1, MAX_PAGE_SIZE].
    #[serde(default = "default_page_size")]
    pub page_size: u16,
    /// Sort direction.
    #[serde(default)]
    pub sort_order: Option<SortOrder>,
}

fn default_page() -> u32 {
    1
}
fn default_page_size() -> u16 {
    DEFAULT_PAGE_SIZE
}

impl PageQuery {
    /// Return validated page (>= 1) and page_size (clamped to [1, MAX_PAGE_SIZE]).
    #[must_use]
    pub fn validated(&self) -> (u32, u16) {
        let page = self.page.max(1);
        let page_size = self.page_size.clamp(1, MAX_PAGE_SIZE);
        (page, page_size)
    }

    /// Compute the SQL OFFSET from validated page and page_size.
    #[must_use]
    pub fn offset(&self) -> i64 {
        let (page, page_size) = self.validated();
        i64::from((page - 1) * u32::from(page_size))
    }

    /// Return the sort order, defaulting to descending.
    #[must_use]
    pub fn sort_order(&self) -> SortOrder {
        self.sort_order.unwrap_or(SortOrder::Desc)
    }
}

impl Default for PageQuery {
    fn default() -> Self {
        Self {
            page: 1,
            page_size: DEFAULT_PAGE_SIZE,
            sort_order: None,
        }
    }
}

/// Pagination metadata included in every paginated response.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct PageMeta {
    pub page: u32,
    pub page_size: u16,
    pub total_items: i64,
    pub total_pages: u32,
}

impl PageMeta {
    /// Build page metadata from a query and a total row count.
    #[must_use]
    pub fn from_total(query: &PageQuery, total_items: i64) -> Self {
        let (page, page_size) = query.validated();
        let ps = i64::from(page_size);
        let total_pages = if total_items == 0 {
            1
        } else {
            u32::try_from((total_items + ps - 1) / ps).unwrap_or(u32::MAX)
        };
        let page = page.min(total_pages).max(1);
        Self {
            page,
            page_size,
            total_items,
            total_pages,
        }
    }
}

/// Generic paginated response envelope.
///
/// Every paginated list endpoint returns this wrapper so the frontend
/// can display page controls consistently.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Paginated<T> {
    pub data: Vec<T>,
    pub page: PageMeta,
}

impl<T> Paginated<T> {
    /// Build a paginated response from items, query, and total count.
    #[must_use]
    pub fn new(data: Vec<T>, query: &PageQuery, total_items: i64) -> Self {
        Self {
            data,
            page: PageMeta::from_total(query, total_items),
        }
    }

    /// Map items through a projection while preserving page metadata.
    #[must_use]
    pub fn map<U>(self, f: impl FnMut(T) -> U) -> Paginated<U> {
        Paginated {
            data: self.data.into_iter().map(f).collect(),
            page: self.page,
        }
    }
}
