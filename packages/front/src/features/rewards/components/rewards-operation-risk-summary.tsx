import type { RewardBotConfigDto, RewardBotSnapshotDto } from "@/lib/contracts/dto";
import { formatUsdFixed } from "@/lib/formatters";
import { dictionary } from "@/lib/i18n/dictionaries";

export type RewardsConfirmKind = "run" | "save" | "cancel" | "reset";
export type RewardRiskySaveScope =
  | "rewards_live_trading_enable"
  | "rewards_merge_auto_execute";

export function operationRequiresStepUp(kind: RewardsConfirmKind): boolean {
  return kind === "run" || kind === "save" || kind === "reset";
}

export function operationRequiresNote(kind: RewardsConfirmKind): boolean {
  return kind === "save" || kind === "reset";
}

export function OperationRiskSummary({
  kind,
  snapshot,
  draft,
  isDirty,
  riskySaveScopes,
}: {
  kind: RewardsConfirmKind;
  snapshot: RewardBotSnapshotDto;
  draft: RewardBotConfigDto;
  isDirty: boolean;
  riskySaveScopes: RewardRiskySaveScope[];
}) {
  const items =
    kind === "run"
      ? [
          snapshot.config.enabled
            ? dictionary.rewards.riskSummaryTradingEnabled
            : dictionary.rewards.riskSummaryTradingDisabled,
          `${dictionary.rewards.maxMarkets}: ${snapshot.config.max_markets} · ${dictionary.rewards.maxOpenOrders}: ${snapshot.config.max_open_orders}`,
          `${dictionary.rewards.makerMarketBudgetUsd}: ${formatUsdFixed(snapshot.config.maker_market_budget_usd)} · ${dictionary.rewards.maxGlobalPositionUsd}: ${formatUsdFixed(snapshot.config.max_global_position_usd)}`,
          snapshot.config.balanced_merge_auto_execute_enabled
            ? dictionary.rewards.riskSummaryAutoMergeEnabled
            : dictionary.rewards.riskSummaryAutoMergeDisabled,
          ...(isDirty ? [dictionary.rewards.riskSummaryUnsavedDraftIgnored] : []),
        ]
      : kind === "save"
        ? [
            ...(riskySaveScopes.includes("rewards_live_trading_enable")
              ? [dictionary.rewards.riskSummaryEnableTrading]
              : []),
            ...(riskySaveScopes.includes("rewards_merge_auto_execute")
              ? [dictionary.rewards.riskSummaryEnableAutoMerge]
              : []),
            `${dictionary.rewards.maxMarkets}: ${draft.max_markets} · ${dictionary.rewards.maxOpenOrders}: ${draft.max_open_orders}`,
            `${dictionary.rewards.makerMarketBudgetUsd}: ${formatUsdFixed(draft.maker_market_budget_usd)} · ${dictionary.rewards.maxGlobalPositionUsd}: ${formatUsdFixed(draft.max_global_position_usd)}`,
          ]
        : kind === "cancel"
          ? [
              `${dictionary.rewards.openOrders}: ${snapshot.status.open_orders}`,
              dictionary.rewards.riskSummaryCancelProtective,
            ]
          : [
              `${dictionary.rewards.openOrders}: ${snapshot.status.open_orders} · ${dictionary.rewards.positions}: ${snapshot.status.positions}`,
              dictionary.rewards.riskSummaryReset,
            ];

  return (
    <div>
      <p className="font-medium text-foreground">{dictionary.rewards.riskSummaryTitle}</p>
      <ul className="mt-2 list-disc space-y-1 pl-5">
        {items.map((item) => (
          <li key={item}>{item}</li>
        ))}
      </ul>
    </div>
  );
}
