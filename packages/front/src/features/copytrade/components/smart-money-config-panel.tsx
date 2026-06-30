"use client";

import { startTransition, useState } from "react";
import { Save, Settings2 } from "lucide-react";
import { toast } from "sonner";

import { OperationFeedbackBanner } from "@/components/shared/operation-feedback-banner";
import { StatusPill } from "@/components/shared/status-pill";
import { Button } from "@/components/ui/button";
import { Card, CardContent, CardDescription, CardHeader, CardTitle } from "@/components/ui/card";
import { Input } from "@/components/ui/input";
import {
  updateSmartMoneyConfigAction,
  type SmartMoneyActionResult,
} from "@/lib/api/actions";
import type {
  RewardAiProvider,
  RewardAiRequestFormat,
  SmartMoneyConfigDto,
  SmartMoneyMode,
  SmartMoneySnapshotDto,
} from "@/lib/contracts/dto";
import { dictionary } from "@/lib/i18n/dictionaries";

type SmartMoneyNumericConfigKey =
  | "min_trade_count"
  | "min_settled_trade_count"
  | "signal_advisory_max_concurrency"
  | "min_total_volume_usd"
  | "min_copyability_score"
  | "max_signal_age_ms"
  | "max_price_slippage_cents"
  | "min_orderbook_depth_usd"
  | "max_wallet_exposure_usd"
  | "max_market_exposure_usd"
  | "max_daily_notional_usd";

const integerConfigKeys = new Set<SmartMoneyNumericConfigKey>([
  "min_trade_count",
  "min_settled_trade_count",
  "signal_advisory_max_concurrency",
  "max_signal_age_ms",
]);

function updateNumericValue(
  config: SmartMoneyConfigDto,
  key: SmartMoneyNumericConfigKey,
  value: string,
): SmartMoneyConfigDto {
  if (integerConfigKeys.has(key)) {
    const numericValue = Number.parseInt(value || "0", 10);
    return { ...config, [key]: Number.isFinite(numericValue) ? numericValue : 0 };
  }

  return { ...config, [key]: value };
}

function ConfigNumberInput({
  label,
  value,
  suffix,
  min = 0,
  max,
  step = "any",
  onChange,
}: {
  label: string;
  value: string | number;
  suffix?: string;
  min?: number;
  max?: number;
  step?: string;
  onChange: (value: string) => void;
}) {
  return (
    <label className="space-y-1.5">
      <span className="text-xs font-medium text-muted-foreground">{label}</span>
      <div className="flex items-center gap-2">
        <Input
          type="number"
          min={min}
          max={max}
          step={step}
          value={value}
          onChange={(event) => onChange(event.target.value)}
          className="h-9 text-xs"
        />
        {suffix ? <span className="w-10 text-xs text-muted-foreground">{suffix}</span> : null}
      </div>
    </label>
  );
}

function ConfigToggle({
  label,
  checked,
  onChange,
}: {
  label: string;
  checked: boolean;
  onChange: (checked: boolean) => void;
}) {
  return (
    <label className="flex items-center justify-between gap-3 rounded-md border border-border/60 px-3 py-2 text-sm">
      <span className="text-muted-foreground">{label}</span>
      <input
        type="checkbox"
        className="size-4 accent-primary"
        checked={checked}
        onChange={(event) => onChange(event.target.checked)}
      />
    </label>
  );
}

export function SmartMoneyConfigPanel({
  snapshot,
  onSnapshotChange,
}: {
  snapshot: SmartMoneySnapshotDto;
  onSnapshotChange: (snapshot: SmartMoneySnapshotDto) => void;
}) {
  const t = dictionary.copytrade.smartConfig;
  const [draft, setDraft] = useState<SmartMoneyConfigDto>(snapshot.config);
  const [feedback, setFeedback] = useState<SmartMoneyActionResult | null>(null);
  const [pending, setPending] = useState(false);

  function applyResult(result: SmartMoneyActionResult) {
    setFeedback(result);
    if (result.ok) {
      toast.success(result.message);
    } else {
      toast.error(result.message);
    }
    if (result.snapshot) {
      onSnapshotChange(result.snapshot);
      setDraft(result.snapshot.config);
    }
  }

  function saveConfig() {
    setPending(true);
    startTransition(() => {
      void updateSmartMoneyConfigAction(draft)
        .then(applyResult)
        .finally(() => setPending(false));
    });
  }

  function setNumber(key: SmartMoneyNumericConfigKey, value: string) {
    setDraft((current) => updateNumericValue(current, key, value));
  }

  return (
    <Card>
      <CardHeader className="flex flex-col gap-4 border-b border-border/70 xl:flex-row xl:items-center xl:justify-between">
        <div>
          <CardTitle className="font-heading text-base">{t.title}</CardTitle>
          <CardDescription>{t.description}</CardDescription>
        </div>
        <div className="flex flex-wrap items-center gap-2">
          <StatusPill tone={snapshot.status.enabled ? "success" : "neutral"}>
            {snapshot.status.enabled ? dictionary.common.enabled : dictionary.common.disabled}
          </StatusPill>
          <StatusPill tone="primary">{t.modeLabels[snapshot.status.mode]}</StatusPill>
          <Button size="sm" disabled={pending} onClick={saveConfig}>
            <Save className="size-4" /> {dictionary.copytrade.save}
          </Button>
        </div>
      </CardHeader>
      <CardContent className="space-y-4">
        {feedback ? <OperationFeedbackBanner feedback={feedback} onDismiss={() => setFeedback(null)} /> : null}

        <div className="grid gap-3 md:grid-cols-2 xl:grid-cols-4">
          <ConfigToggle
            label={t.enabled}
            checked={draft.enabled}
            onChange={(checked) => setDraft((current) => ({ ...current, enabled: checked }))}
          />
          <ConfigToggle
            label={t.discoveryEnabled}
            checked={draft.discovery_enabled}
            onChange={(checked) =>
              setDraft((current) => ({ ...current, discovery_enabled: checked }))
            }
          />
          <ConfigToggle
            label={t.walletAdvisoryEnabled}
            checked={draft.wallet_advisory_enabled}
            onChange={(checked) =>
              setDraft((current) => ({ ...current, wallet_advisory_enabled: checked }))
            }
          />
          <ConfigToggle
            label={t.signalAdvisoryEnabled}
            checked={draft.signal_advisory_enabled}
            onChange={(checked) =>
              setDraft((current) => ({ ...current, signal_advisory_enabled: checked }))
            }
          />
          <ConfigToggle
            label={t.signalAdvisoryConcurrencyEnabled}
            checked={draft.signal_advisory_concurrency_enabled}
            onChange={(checked) =>
              setDraft((current) => ({
                ...current,
                signal_advisory_concurrency_enabled: checked,
              }))
            }
          />
        </div>

        <label className="block space-y-1.5">
          <span className="flex items-center gap-2 text-xs font-medium text-muted-foreground">
            <Settings2 className="size-3" /> {t.mode}
          </span>
          <select
            className="h-9 w-full rounded-md border border-input bg-background px-3 text-sm text-foreground"
            value={draft.mode}
            onChange={(event) =>
              setDraft((current) => ({ ...current, mode: event.target.value as SmartMoneyMode }))
            }
          >
            <option value="observe">{t.modeLabels.observe}</option>
            <option value="paper">{t.modeLabels.paper}</option>
            <option value="approval">{t.modeLabels.approval}</option>
            <option value="live_guarded">{t.modeLabels.live_guarded}</option>
          </select>
        </label>

        <div className="grid gap-3 md:grid-cols-3">
          <label className="block space-y-1.5">
            <span className="text-xs font-medium text-muted-foreground">
              {t.signalAdvisoryProvider}
            </span>
            <select
              className="h-9 w-full rounded-md border border-input bg-background px-3 text-sm text-foreground"
              value={draft.signal_advisory_provider}
              onChange={(event) =>
                setDraft((current) => ({
                  ...current,
                  signal_advisory_provider: event.target.value as RewardAiProvider,
                  signal_advisory_request_format:
                    event.target.value === "anthropic"
                      ? "anthropic_messages"
                      : current.signal_advisory_request_format === "anthropic_messages"
                        ? "openai_responses"
                        : current.signal_advisory_request_format,
                }))
              }
            >
              <option value="openai">{t.providerLabels.openai}</option>
              <option value="anthropic">{t.providerLabels.anthropic}</option>
            </select>
          </label>

          <label className="block space-y-1.5">
            <span className="text-xs font-medium text-muted-foreground">
              {t.signalAdvisoryRequestFormat}
            </span>
            <select
              className="h-9 w-full rounded-md border border-input bg-background px-3 text-sm text-foreground"
              value={draft.signal_advisory_request_format}
              onChange={(event) =>
                setDraft((current) => ({
                  ...current,
                  signal_advisory_request_format: event.target.value as RewardAiRequestFormat,
                }))
              }
            >
              {draft.signal_advisory_provider === "anthropic" ? (
                <option value="anthropic_messages">
                  {t.requestFormatLabels.anthropic_messages}
                </option>
              ) : (
                <>
                  <option value="openai_responses">
                    {t.requestFormatLabels.openai_responses}
                  </option>
                  <option value="openai_chat_completions">
                    {t.requestFormatLabels.openai_chat_completions}
                  </option>
                </>
              )}
            </select>
          </label>

          <label className="block space-y-1.5">
            <span className="text-xs font-medium text-muted-foreground">
              {t.signalAdvisoryModel}
            </span>
            <Input
              value={draft.signal_advisory_model}
              onChange={(event) =>
                setDraft((current) => ({
                  ...current,
                  signal_advisory_model: event.target.value,
                }))
              }
              className="h-9 text-xs"
            />
          </label>

          <ConfigNumberInput
            label={t.signalAdvisoryMaxConcurrency}
            value={draft.signal_advisory_max_concurrency}
            min={1}
            max={10}
            step="1"
            onChange={(value) => setNumber("signal_advisory_max_concurrency", value)}
          />
        </div>

        <div className="grid gap-3 md:grid-cols-2 xl:grid-cols-5">
          <ConfigNumberInput
            label={t.minTradeCount}
            value={draft.min_trade_count}
            step="1"
            onChange={(value) => setNumber("min_trade_count", value)}
          />
          <ConfigNumberInput
            label={t.minSettledTradeCount}
            value={draft.min_settled_trade_count}
            step="1"
            onChange={(value) => setNumber("min_settled_trade_count", value)}
          />
          <ConfigNumberInput
            label={t.minTotalVolumeUsd}
            value={draft.min_total_volume_usd}
            suffix="$"
            onChange={(value) => setNumber("min_total_volume_usd", value)}
          />
          <ConfigNumberInput
            label={t.minCopyabilityScore}
            value={draft.min_copyability_score}
            max={1}
            step="0.01"
            onChange={(value) => setNumber("min_copyability_score", value)}
          />
          <ConfigNumberInput
            label={t.maxSignalAgeMs}
            value={draft.max_signal_age_ms}
            suffix="ms"
            min={1000}
            step="1000"
            onChange={(value) => setNumber("max_signal_age_ms", value)}
          />
          <ConfigNumberInput
            label={t.maxPriceSlippageCents}
            value={draft.max_price_slippage_cents}
            suffix="c"
            onChange={(value) => setNumber("max_price_slippage_cents", value)}
          />
          <ConfigNumberInput
            label={t.minOrderbookDepthUsd}
            value={draft.min_orderbook_depth_usd}
            suffix="$"
            onChange={(value) => setNumber("min_orderbook_depth_usd", value)}
          />
          <ConfigNumberInput
            label={t.maxWalletExposureUsd}
            value={draft.max_wallet_exposure_usd}
            suffix="$"
            onChange={(value) => setNumber("max_wallet_exposure_usd", value)}
          />
          <ConfigNumberInput
            label={t.maxMarketExposureUsd}
            value={draft.max_market_exposure_usd}
            suffix="$"
            onChange={(value) => setNumber("max_market_exposure_usd", value)}
          />
          <ConfigNumberInput
            label={t.maxDailyNotionalUsd}
            value={draft.max_daily_notional_usd}
            suffix="$"
            onChange={(value) => setNumber("max_daily_notional_usd", value)}
          />
        </div>

        <p className="text-xs text-muted-foreground">{t.executionNote}</p>
      </CardContent>
    </Card>
  );
}
