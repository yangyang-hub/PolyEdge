export const dashboard = {
  dashboard: {
    eyebrow: "运行台",
    title: "仪表盘",
    description: "汇总市场、事件和新闻源状态。",
    streamTitle: "后端数据已同步",
    streamDetail: "页面通过 REST API 读取数据库和 orderbook 服务中的当前状态。",
    newsHealthTitle: "新闻源健康检查",
    newsHealthDetail: "新闻采集和事件提升链路仍是控制台的主要观察入口。",
    hotMarkets: "热门市场",
    latestEvents: "最新事件",
    newsSources: "新闻源",
    degradedSources: "个源降级",
  },
  metrics: {
    coveredMarkets: "覆盖市场",
    tradableMarkets: "可交易市场",
    activeEvents: "活跃事件",
    newsSources: "新闻源",
  },
} as const;
