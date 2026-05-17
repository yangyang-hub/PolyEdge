import { formatMessage, type Dictionary } from "@/lib/i18n/dictionaries";
import type { Locale } from "@/lib/i18n/locales";

type GeneratedCopyKey = keyof Dictionary["generated"];

const exactGeneratedCopyKeys: Record<string, GeneratedCopyKey> = {
  "Approval Queue": "approvalQueue",
  "Data Layer": "dataLayer",
  "ETH ETF Theme": "ethEtfTheme",
  "Global Risk": "globalRisk",
  "Latency Watch": "latencyWatch",
  "Operator Desk": "operatorDesk",
  "Ops Desk": "opsDesk",
  "Risk Admin": "riskAdmin",
  "Risk Engine": "riskEngine",
  "Signal is queued for manual review because settlement ambiguity is medium and theme exposure is elevated.":
    "signalQueuedManualReview",
  "ETH staking ETF short requires manual confirmation due to ambiguity.":
    "ethStakingManualConfirmation",
  "Request to switch runtime from paper_trade to manual_confirm.":
    "requestRuntimeSwitch",
  "Potential market data stale condition across macro markets.":
    "potentialStaleMarketData",
  "BTC breakout long was approved for automatic release after operator review.":
    "btcApproved",
  "Request to switch into live auto was rejected because risk alerts were unresolved.":
    "liveAutoRejected",
  "Daily loss usage reached 84% of configured limit.": "dailyLossUsage",
  "Stale market snapshots detected in one macro source.": "staleMarketSnapshots",
  "Event cluster exposure above preferred threshold.": "eventClusterExposure",
  "Eligible for automated execution under current bucket limits.": "signalAutoEligible",
  "Watch only until confidence recovers above activation threshold.": "watchConfidence",
  "Reversed to manual monitoring because upstream data quality is degraded.":
    "reversedManualMonitoring",
  "ETF inflow narrative still supports underpriced upside participation.":
    "etfInflowNarrative",
  "Official update increases review-delay odds more than current price reflects.":
    "officialUpdateReviewDelay",
  "Momentum evidence remains directionally positive but confidence decayed after contradictory flow.":
    "momentumEvidence",
  "Macro drift remains negative for cuts, but live macro feed instability invalidates autonomous posture.":
    "macroDrift",
  "Unknown / manual review": "unknownManualReviewContext",
};

export function localizeGeneratedCopy(
  locale: Locale,
  dictionary: Dictionary,
  value: string,
): string {
  if (locale !== "zh-CN") {
    return value;
  }

  const normalizedValue = value.trim();
  const exactKey = exactGeneratedCopyKeys[normalizedValue];

  if (exactKey) {
    return dictionary.generated[exactKey];
  }

  const approvalItemsMatch = normalizedValue.match(
    /^(\d+) signal approval items? await operator review\.$/,
  );

  if (approvalItemsMatch) {
    return formatMessage(dictionary.generated.signalApprovalItemsAwaitReview, {
      count: approvalItemsMatch[1],
    });
  }

  const manualConfirmationMatch = normalizedValue.match(
    /^(.+) requires manual confirmation\. Signal is queued for manual review because settlement ambiguity is medium and theme exposure is elevated\.$/,
  );

  if (manualConfirmationMatch) {
    return formatMessage(dictionary.generated.requiresManualConfirmationSummary, {
      subject: manualConfirmationMatch[1],
      decision: dictionary.generated.signalQueuedManualReview,
    });
  }

  return normalizedValue
    .replaceAll("manual review", dictionary.enums.manual_review)
    .replaceAll("observe only", dictionary.enums.observe_only)
    .replaceAll("tradable", dictionary.enums.tradable);
}
