use polyedge_application::OrderbookSubscriptionRegistry;
use std::time::Instant;

/// Maximum distinct sources the registry retains. Enforced atomically inside
/// `register_tokens` (the HTTP layer also rejects early with a friendlier 400).
const MAX_REGISTRY_SOURCES: usize = 32;

/// 基于内存的订阅注册中心实现。
///
/// 每个 `source` 维护独立的有序 token 集合。注册会原子替换来源集合，
/// `list_all_tokens` 按 live rewards、execution、candidate、copytrade 优先级返回并集。
pub struct InMemoryOrderbookSubscriptionRegistry {
    /// source -> ordered token_ids
    tokens: RwLock<HashMap<String, Vec<String>>>,
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
    async fn register_tokens(&self, source: &str, token_ids: &[String]) -> Result<()> {
        let mut seen = HashSet::new();
        let replacement = token_ids
            .iter()
            .filter(|id| !id.trim().is_empty() && seen.insert((*id).clone()))
            .cloned()
            .collect::<Vec<_>>();
        let mut tokens = self.tokens.write().await;
        // Enforce the source cap atomically under the write lock: a brand-new
        // source registering while already at capacity is rejected. Doing this
        // here (rather than a separate count-then-insert in the caller) closes
        // the check-then-act race that could push the registry past the cap.
        if !replacement.is_empty()
            && !tokens.contains_key(source)
            && tokens.len() >= MAX_REGISTRY_SOURCES
        {
            return Err(polyedge_domain::AppError::invalid_input(
                "ORDERBOOK_REGISTRY_FULL",
                format!("orderbook registry supports at most {MAX_REGISTRY_SOURCES} sources"),
            ));
        }
        let changed = if replacement.is_empty() {
            tokens.remove(source).is_some()
        } else if tokens.get(source) == Some(&replacement) {
            false
        } else {
            tokens.insert(source.to_string(), replacement);
            true
        };
        drop(tokens);
        if changed {
            self.touch().await;
        }
        Ok(())
    }

    async fn unregister_source(&self, source: &str) -> Result<()> {
        let mut tokens = self.tokens.write().await;
        let removed = tokens.remove(source).is_some();
        drop(tokens);
        if removed {
            self.touch().await;
        }
        Ok(())
    }

    async fn unregister_tokens(&self, source: &str, token_ids: &[String]) -> Result<()> {
        let mut tokens = self.tokens.write().await;
        if let Some(entry) = tokens.get_mut(source) {
            let before = entry.len();
            entry.retain(|id| !token_ids.contains(id));
            let removed = before.saturating_sub(entry.len());
            if entry.is_empty() {
                tokens.remove(source);
            }
            drop(tokens);
            if removed > 0 {
                self.touch().await;
            }
        }
        Ok(())
    }

    async fn list_all_tokens(&self) -> Vec<String> {
        let tokens = self.tokens.read().await;
        let mut sources = tokens.iter().collect::<Vec<_>>();
        sources.sort_by(|(left, _), (right, _)| {
            registry_source_priority(left)
                .cmp(&registry_source_priority(right))
                .then_with(|| left.cmp(right))
        });
        let mut seen = HashSet::new();
        let mut result = Vec::new();
        for (_, entry) in sources {
            for id in entry {
                if seen.insert(id.clone()) {
                    result.push(id.clone());
                }
            }
        }
        result
    }

    async fn total_token_count(&self) -> usize {
        let tokens = self.tokens.read().await;
        let mut seen = HashSet::new();
        for entry in tokens.values() {
            for id in entry {
                seen.insert(id);
            }
        }
        seen.len()
    }

    async fn source_count(&self) -> usize {
        let tokens = self.tokens.read().await;
        tokens.len()
    }

    async fn has_source(&self, source: &str) -> bool {
        let tokens = self.tokens.read().await;
        tokens.contains_key(source)
    }

    async fn changed_since(&self, since: Instant) -> bool {
        let ts = self.last_modified.lock().await;
        *ts > since
    }
}

fn registry_source_priority(source: &str) -> u8 {
    match source {
        "rewards_active" => 0,
        "exec_orders" => 1,
        "rewards_eligible" => 2,
        "rewards" | "rewards_candidates" => 3,
        "copytrade" => 4,
        _ => 5,
    }
}
