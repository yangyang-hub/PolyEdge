import { getConsoleAuthMode } from "@/lib/console-auth";
import { dictionary, translateEnum, formatMessage } from "@/lib/i18n/dictionaries";
import {
  formatClock,
  formatInteger,
  formatOptionalClock,
  formatPercentFromRatio,
  toFiniteNumber,
  type Tone,
} from "@/lib/formatters";
import { getBackendMode, getConfiguredApiBaseUrl } from "@/lib/api/base";
import { listNewsRawEvents, listNewsSourceHealth } from "@/lib/api/news";
import { readRuntimeConfig } from "@/lib/api/settings";

function sourceHealthTone(healthScore: string, consecutiveFailures: number): Tone {
  const score = toFiniteNumber(healthScore);

  if (!Number.isFinite(score) || score < 0.5 || consecutiveFailures >= 3) {
    return "danger";
  }

  if (score < 0.75 || consecutiveFailures > 0) {
    return "warning";
  }

  return "success";
}

export async function getSettingsPageData() {
  const [{ data: sourceHealth }, { data: rawNews }, { data: runtimeConfig }] = await Promise.all([
    listNewsSourceHealth({ limit: 10 }),
    listNewsRawEvents({ limit: 8 }),
    readRuntimeConfig(),
  ]);
  const degradedSources = sourceHealth.filter(
    (source) => sourceHealthTone(source.health_score, source.consecutive_failures) !== "success",
  );

  return {
    backendMode: getBackendMode(),
    apiBaseUrl: getConfiguredApiBaseUrl() ?? "same-origin /api/v1",
    consoleAuthMode: getConsoleAuthMode(process.env.NEXT_PUBLIC_POLYEDGE_CONSOLE_AUTH),
    runtimeConfig,
    sourceHealthSummary: {
      label: degradedSources.length > 0
        ? formatMessage(dictionary.settings.degraded, { count: degradedSources.length })
        : dictionary.common.healthy,
      tone: degradedSources.length > 0 ? ("warning" as const) : ("success" as const),
    },
    sourceHealth: sourceHealth.map((source) => ({
      source: source.source,
      typeLabel: translateEnum(source.source_type),
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
      typeLabel: translateEnum(event.source_type),
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
