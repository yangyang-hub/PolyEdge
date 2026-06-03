use polyedge_application::OrderbookSubscriptionRegistry;
use std::collections::HashSet;
use std::time::Instant;

/// 基于内存的订阅注册中心实现。
///
/// 每个 `source`（如 "rewards"、"copytrade"、"exec_orders"）维护独立的 token 集合。
/// `list_all_tokens` 返回所有来源的并集；`changed_since` 用于增量判断 WS 是否需要重连。
pub struct InMemoryOrderbookSubscriptionRegistry {
    /// source -> set of token_ids
    tokens: RwLock<HashMap<String, HashSet<String>>>,
    /// 最后一次写操作的时间戳
    last_modified: Mutex<Instant>,
}

impl InMemoryOrderbookSubscriptionRegistry {
    pub fn new() -> Self {
        Self {
            tokens: RwLock::new(HashMap::new()),
            last_modified: Mutex::new(Instant::now()),
        }
    }

    async fn touch(&self) {
        let mut ts = self.last_modified.lock().await;
        *ts = Instant::now();
    }
}

#[async_trait]
impl OrderbookSubscriptionRegistry for InMemoryOrderbookSubscriptionRegistry {
    async fn register_tokens(&self, source: &str, token_ids: &[String]) {
        let mut tokens = self.tokens.write().await;
        let entry = tokens.entry(source.to_string()).or_default();
        let before = entry.len();
        for id in token_ids {
            entry.insert(id.clone());
        }
        let added = entry.len().saturating_sub(before);
        drop(tokens);
        if added > 0 {
            self.touch().await;
        }
    }

    async fn unregister_source(&self, source: &str) {
        let mut tokens = self.tokens.write().await;
        let removed = tokens.remove(source).is_some();
        drop(tokens);
        if removed {
            self.touch().await;
        }
    }

    async fn unregister_tokens(&self, source: &str, token_ids: &[String]) {
        let mut tokens = self.tokens.write().await;
        if let Some(entry) = tokens.get_mut(source) {
            let before = entry.len();
            for id in token_ids {
                entry.remove(id);
            }
            let removed = before.saturating_sub(entry.len());
            if entry.is_empty() {
                tokens.remove(source);
            }
            drop(tokens);
            if removed > 0 {
                self.touch().await;
            }
        }
    }

    async fn list_all_tokens(&self) -> Vec<String> {
        let tokens = self.tokens.read().await;
        let mut seen = HashSet::new();
        let mut result = Vec::new();
        for entry in tokens.values() {
            for id in entry {
                if seen.insert(id.clone()) {
                    result.push(id.clone());
                }
            }
        }
        result
    }

    async fn source_count(&self) -> usize {
        let tokens = self.tokens.read().await;
        tokens.len()
    }

    async fn changed_since(&self, since: Instant) -> bool {
        let ts = self.last_modified.lock().await;
        *ts > since
    }
}
