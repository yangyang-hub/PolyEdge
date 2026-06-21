use async_trait::async_trait;
use polyedge_domain::Result;
use std::time::Instant;
use tokio::sync::watch;

/// 订阅注册中心 — 任何服务可以注册/注销感兴趣的 token ID。
///
/// Orderbook stream worker 从注册中心聚合所有 token，去重后统一 WS 订阅。
/// 新消费者只需调用 `register_tokens`，无需修改 stream worker 代码。
#[async_trait]
pub trait OrderbookSubscriptionRegistry: Send + Sync {
    /// Atomically replace a source's ordered token set.
    async fn register_tokens(&self, source: &str, token_ids: &[String]) -> Result<()>;

    /// 注销某来源的所有 token。
    async fn unregister_source(&self, source: &str) -> Result<()>;

    /// 注销某来源的指定 token。
    async fn unregister_tokens(&self, source: &str, token_ids: &[String]) -> Result<()>;

    /// 返回当前所有已注册的去重 token 列表。
    async fn list_all_tokens(&self) -> Vec<String>;

    /// Return the current ordered token set for one source.
    async fn list_source_tokens(&self, source: &str) -> Vec<String>;

    /// 返回去重 token 总数（不需要构建完整列表）。
    async fn total_token_count(&self) -> usize;

    /// 返回当前注册来源数量。
    async fn source_count(&self) -> usize;

    /// 返回指定来源当前是否存在。
    async fn has_source(&self, source: &str) -> bool;

    /// 返回自某个时间点之后是否有变更（用于增量判断是否需要 WS 重连）。
    async fn changed_since(&self, since: Instant) -> bool;

    /// Subscribe to registry changes when the implementation is local.
    fn subscribe_changes(&self) -> Option<watch::Receiver<u64>> {
        None
    }
}
