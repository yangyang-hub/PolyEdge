import "server-only";

import { getConsoleAuthMode } from "@/lib/console-auth";
import {
  formatClock,
  formatInteger,
  formatPercentFromRatio,
  humanizeSnakeCase,
  type Tone,
} from "@/lib/server/console-formatters";
import { getApiBaseUrl, getBackendMode } from "@/server/api/base";
import { listNewsSourceHealth } from "@/server/api/news";

function sourceHealthTone(healthScore: string, consecutiveFailures: number): Tone {
  const score = Number.parseFloat(healthScore);

  if (!Number.isFinite(score) || score < 0.5 || consecutiveFailures >= 3) {
    return "danger";
  }

  if (score < 0.75 || consecutiveFailures > 0) {
    return "warning";
  }

  return "success";
}

function formatOptionalClock(value: string | null | undefined): string {
  return value ? formatClock(value) : "none";
}

export async function getSettingsPageData() {
  const { data: sourceHealth } = await listNewsSourceHealth({ limit: 10 });
  const degradedSources = sourceHealth.filter(
    (source) => sourceHealthTone(source.health_score, source.consecutive_failures) !== "success",
  );

  return {
    backendMode: getBackendMode(),
    apiBaseUrl: getApiBaseUrl(),
    consoleAuthMode: getConsoleAuthMode(process.env.POLYEDGE_CONSOLE_AUTH),
    sourceHealthSummary: {
      label: degradedSources.length > 0 ? `${degradedSources.length} degraded` : "healthy",
      tone: degradedSources.length > 0 ? ("warning" as const) : ("success" as const),
    },
    sourceHealth: sourceHealth.map((source) => ({
      source: source.source,
      typeLabel: humanizeSnakeCase(source.source_type),
      enabledLabel: source.enabled ? "enabled" : "disabled",
      healthScoreLabel: formatPercentFromRatio(source.health_score),
      healthScoreWidth: formatPercentFromRatio(source.health_score),
      reliabilityLabel: formatPercentFromRatio(source.reliability),
      fetchedLabel: formatInteger(String(source.items_fetched)),
      insertedLabel: formatInteger(String(source.items_inserted)),
      dedupedLabel: formatInteger(String(source.items_deduped)),
      consecutiveFailures: source.consecutive_failures,
      lastSuccessLabel: formatOptionalClock(source.last_success_at),
      lastErrorLabel: formatOptionalClock(source.last_error_at),
      lastError: source.last_error,
      updatedAtLabel: formatClock(source.updated_at),
      tone: sourceHealthTone(source.health_score, source.consecutive_failures),
    })),
  };
}
