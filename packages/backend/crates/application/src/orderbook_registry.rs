use async_trait::async_trait;
use std::time::Instant;

/// 订阅注册中心 — 任何服务可以注册/注销感兴趣的 token ID。
///
/// Orderbook stream worker 从注册中心聚合所有 token，去重后统一 WS 订阅。
/// 新消费者只需调用 `register_tokens`，无需修改 stream worker 代码。
#[async_trait]
pub trait OrderbookSubscriptionRegistry: Send + Sync {
    /// 注册一组 token ID（幂等，重复注册不产生副作用）。
    async fn register_tokens(&self, source: &str, token_ids: &[String]);

    /// 注销某来源的所有 token。
    async fn unregister_source(&self, source: &str);

    /// 注销某来源的指定 token。
    async fn unregister_tokens(&self, source: &str, token_ids: &[String]);

    /// 返回当前所有已注册的去重 token 列表。
    async fn list_all_tokens(&self) -> Vec<String>;

    /// 返回自某个时间点之后是否有变更（用于增量判断是否需要 WS 重连）。
    async fn changed_since(&self, since: Instant) -> bool;
}
