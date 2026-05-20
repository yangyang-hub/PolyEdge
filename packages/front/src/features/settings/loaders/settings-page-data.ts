import "server-only";

import { getConsoleAuthMode } from "@/lib/console-auth";
import { getServerI18n } from "@/lib/i18n/server";
import {
  formatClock,
  formatInteger,
  formatPercentFromRatio,
  type Tone,
} from "@/lib/server/console-formatters";
import { getBackendMode, getConfiguredApiBaseUrl } from "@/server/api/base";
import { listNewsRawEvents, listNewsSourceHealth } from "@/server/api/news";

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

function formatOptionalClock(value: string | null | undefined, fallback: string): string {
  return value ? formatClock(value) : fallback;
}

export async function getSettingsPageData() {
  const [{ data: sourceHealth }, { data: rawNews }, i18n] = await Promise.all([
    listNewsSourceHealth({ limit: 10 }),
    listNewsRawEvents({ limit: 8 }),
    getServerI18n(),
  ]);
  const { dictionary, enumLabel, format } = i18n;
  const degradedSources = sourceHealth.filter(
    (source) => sourceHealthTone(source.health_score, source.consecutive_failures) !== "success",
  );

  return {
    backendMode: getBackendMode(),
    apiBaseUrl: getConfiguredApiBaseUrl(),
    consoleAuthMode: getConsoleAuthMode(process.env.POLYEDGE_CONSOLE_AUTH),
    sourceHealthSummary: {
      label: degradedSources.length > 0
        ? format(dictionary.settings.degraded, { count: degradedSources.length })
        : dictionary.common.healthy,
      tone: degradedSources.length > 0 ? ("warning" as const) : ("success" as const),
    },
    sourceHealth: sourceHealth.map((source) => ({
      source: source.source,
      typeLabel: enumLabel(source.source_type),
      enabledLabel: source.enabled ? dictionary.common.enabled : dictionary.common.disabled,
      healthScoreLabel: formatPercentFromRatio(source.health_score),
      healthScoreWidth: formatPercentFromRatio(source.health_score),
      reliabilityLabel: formatPercentFromRatio(source.reliability),
      fetchedLabel: formatInteger(String(source.items_fetched)),
      insertedLabel: formatInteger(String(source.items_inserted)),
      dedupedLabel: formatInteger(String(source.items_deduped)),
      consecutiveFailures: source.consecutive_failures,
      lastSuccessLabel: formatOptionalClock(source.last_success_at, dictionary.common.none),
      lastErrorLabel: formatOptionalClock(source.last_error_at, dictionary.common.none),
      lastError: source.last_error,
      updatedAtLabel: formatClock(source.updated_at),
      tone: sourceHealthTone(source.health_score, source.consecutive_failures),
    })),
    rawNews: rawNews.map((event) => ({
      id: event.id,
      source: event.source,
      typeLabel: enumLabel(event.source_type),
      title: event.title,
      url: event.url,
      externalId: event.external_id,
      author: event.author,
      eventTimeLabel: formatClock(event.event_time),
      publishedAtLabel: formatOptionalClock(event.published_at, dictionary.common.none),
      ingestedAtLabel: formatClock(event.ingested_at),
      traceId: event.trace_id,
    })),
  };
}
