import { formatFixed, formatUsdFixed, toFiniteNumber } from "@/lib/formatters";
import { dictionary } from "@/lib/i18n/dictionaries";

export function formatBps(value: string | number | null | undefined, digits = 2) {
  return `${formatFixed(toFiniteNumber(value) / 100, digits)}%`;
}

export function formatOptionalBps(value: string | number | null | undefined) {
  return value == null ? dictionary.rewards.notAvailable : formatBps(value);
}

export function formatOptionalUsd(value: string | number | null | undefined) {
  return value == null ? dictionary.rewards.notAvailable : formatUsdFixed(value);
}

export function formatOptionalCents(value: string | number | null | undefined) {
  return value == null ? dictionary.rewards.notAvailable : `${formatFixed(value, 2)}c`;
}

export function formatOptionalMultiple(value: string | number | null | undefined) {
  return value == null ? dictionary.rewards.notAvailable : `${formatFixed(value, 2)}x`;
}

export function formatLowCompetitionReason(reason: string) {
  const labels = dictionary.rewards.lowCompetitionReasonLabels;
  const normalized = reason.toLowerCase();
  if (normalized.includes("max open orders is zero")) return labels.openOrdersDisabled;
  if (normalized.includes("requires ai advisory")) return labels.requiresAi;
  if (normalized.includes("requires info-risk enforce")) return labels.requiresInfoRisk;
  if (normalized.includes("qualified competition")) return labels.externalCompetition;
  if (normalized.includes("competition share")) return labels.probeShare;
  if (normalized.includes("competition multiple")) return labels.competitionMultiple;
  if (normalized.includes("account allocation")) return labels.accountAllocation;
  if (normalized.includes("condition allocation")) return labels.marketAllocation;
  if (normalized.includes("estimated reward/100/day")) return labels.reward;
  if (normalized.includes("exit depth")) return labels.exitDepth;
  if (normalized.includes("entry exit slippage")) return labels.entryExitSlippage;
  if (normalized.includes("exit slippage")) return labels.exitSlippage;
  if (normalized.includes("bad-fill recovery")) return labels.recoveryDays;
  if (normalized.includes("book history samples")) return labels.samples;
  if (normalized.includes("book history midpoint range unavailable")) return labels.midpointUnavailable;
  if (normalized.includes("midpoint range")) return labels.midpointRange;
  if (normalized.includes("top-of-book flips")) return labels.topOfBookFlips;
  if (normalized.includes("live orderbook validation failed")) return labels.liveValidation;
  if (normalized.includes("recent live orderbook validation failed")) return labels.liveValidation;
  return reason;
}

export function formatLowCompetitionReportReason(reason: string) {
  const labels = dictionary.rewards.lowCompetitionReportReasonLabels;
  const normalized = reason.toLowerCase();
  if (normalized.includes("support considering")) return labels.ready;
  if (normalized.includes("observations")) return labels.observations;
  if (normalized.includes("unique markets")) return labels.uniqueMarkets;
  if (normalized.includes("gate pass ratio")) return labels.gatePass;
  if (normalized.includes("sample insufficiency")) return labels.samples;
  if (normalized.includes("ai block ratio")) return labels.aiBlocked;
  if (normalized.includes("info-risk block ratio")) return labels.infoRiskBlocked;
  if (normalized.includes("median competition share")) return labels.probeShare;
  if (normalized.includes("account allocation p90")) return labels.accountAllocation;
  if (normalized.includes("market allocation p90")) return labels.marketAllocation;
  if (normalized.includes("median estimated reward")) return labels.reward;
  if (normalized.includes("median exit depth multiple")) return labels.exitDepth;
  if (normalized.includes("bad-fill recovery days p95")) return labels.recoveryDays;
  if (normalized.includes("midpoint range p95")) return labels.midpointRange;
  if (normalized.includes("unavailable")) return labels.unavailable;
  return reason;
}
