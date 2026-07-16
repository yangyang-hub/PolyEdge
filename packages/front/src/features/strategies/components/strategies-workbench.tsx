"use client";

import { useEffect, useState, useTransition } from "react";

import { OperationFeedbackBanner } from "@/components/shared/operation-feedback-banner";
import { PageHeader } from "@/components/shared/page-header";
import { StatusPill } from "@/components/shared/status-pill";
import { Button } from "@/components/ui/button";
import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card";
import { Input } from "@/components/ui/input";
import { saveStrategy, type OperationActionResult } from "@/lib/api/actions";
import { listStrategies } from "@/lib/api/strategies";
import type {
  CreateMarketStrategyRequest,
  MarketStrategyData,
  QuotePricingMode,
  QuoteSlotInput,
} from "@/lib/contracts/dto";
import { dictionary, translateEnum } from "@/lib/i18n/dictionaries";
import { canWriteMarkets, useAuth } from "@/components/shared/auth-provider";

type StrategyForm = CreateMarketStrategyRequest;

function blankSlot(index: number): QuoteSlotInput {
  return {
    slot_key: `yes-${index}`,
    outcome: "yes",
    quantity: "10",
    pricing_mode: "book_rank",
    book_rank: 1,
    price_offset: "0",
    minimum_price: "0.01",
    maximum_price: "0.99",
    post_only: true,
    enabled: true,
  };
}

const INITIAL_FORM: StrategyForm = {
  name: "",
  visibility: "private",
  active_from: new Date().toISOString(),
  active_until: new Date(Date.now() + 5 * 60 * 60 * 1000).toISOString(),
  market: {
    condition_id: "",
    slug: "",
    question: "",
    yes_token_id: "",
    no_token_id: "",
  },
  version: {
    reward_minimum_size: "5",
    reward_maximum_spread: "0.03",
    book_freshness_ms: 5_000,
    downward_reprice_confirm_ms: 1_000,
    upward_reprice_confirm_ms: 3_000,
    reprice_cooldown_ms: 10_000,
    max_replaces_per_cycle: 2,
    quote_slots: [blankSlot(1)],
  },
  wallet_ids: [],
};

export function StrategiesWorkbench() {
  const d = dictionary.strategies;
  const { user } = useAuth();
  const writable = canWriteMarkets(user?.role);
  const [form, setForm] = useState<StrategyForm>(INITIAL_FORM);
  const [walletIds, setWalletIds] = useState("");
  const [strategies, setStrategies] = useState<MarketStrategyData[]>([]);
  const [feedback, setFeedback] = useState<OperationActionResult | null>(null);
  const [loadError, setLoadError] = useState("");
  const [isPending, startTransition] = useTransition();

  const reload = () => {
    void listStrategies()
      .then((response) => {
        setStrategies(response.data);
        setLoadError("");
      })
      .catch(() => setLoadError(d.loadFailed));
  };

  useEffect(reload, [d.loadFailed]);

  const setMarket = (key: keyof StrategyForm["market"], value: string) => {
    setForm((current) => ({
      ...current,
      market: { ...current.market, [key]: value },
    }));
  };

  const setVersion = <K extends keyof StrategyForm["version"]>(
    key: K,
    value: StrategyForm["version"][K],
  ) => setForm((current) => ({ ...current, version: { ...current.version, [key]: value } }));

  const updateSlot = (index: number, patch: Partial<QuoteSlotInput>) => {
    setVersion(
      "quote_slots",
      form.version.quote_slots.map((slot, slotIndex) =>
        slotIndex === index ? { ...slot, ...patch } : slot,
      ),
    );
  };

  const setPricingMode = (index: number, pricingMode: QuotePricingMode) => {
    updateSlot(
      index,
      pricingMode === "fixed"
        ? { pricing_mode: "fixed", fixed_price: "0.50", book_rank: undefined }
        : { pricing_mode: "book_rank", fixed_price: undefined, book_rank: 1 },
    );
  };

  const submit = () => {
    startTransition(async () => {
      const result = await saveStrategy({
        ...form,
        market: {
          ...form.market,
          polymarket_url: form.market.polymarket_url?.trim() || undefined,
        },
        version: {
          ...form.version,
          reward_daily_rate: form.version.reward_daily_rate?.trim() || undefined,
        },
        wallet_ids: parseIds(walletIds),
        operator_note: form.operator_note?.trim() || undefined,
      });
      setFeedback(result);
      if (result.ok) reload();
    });
  };

  return (
    <div className="space-y-8">
      <PageHeader eyebrow={d.eyebrow} title={d.title} description={d.description} />
      {feedback ? <OperationFeedbackBanner feedback={feedback} /> : null}

      {writable ? <Card>
        <CardHeader>
          <CardTitle>{d.identity}</CardTitle>
        </CardHeader>
        <CardContent className="grid gap-4 md:grid-cols-2">
          <Field label={d.strategyName} value={form.name} onChange={(value) => setForm((current) => ({ ...current, name: value }))} />
          <SelectField label="可见性" value={form.visibility} onChange={(value) => setForm((current) => ({ ...current, visibility: value as "private" | "followable" }))} options={["private", "followable"]} />
          <Field label="开始生效" type="datetime-local" value={toLocalInput(form.active_from)} onChange={(value) => setForm((current) => ({ ...current, active_from: new Date(value).toISOString() }))} />
          <Field label="停止做市" type="datetime-local" value={toLocalInput(form.active_until)} onChange={(value) => setForm((current) => ({ ...current, active_until: new Date(value).toISOString() }))} />
          <Field label={d.slug} value={form.market.slug} onChange={(value) => setMarket("slug", value)} />
          <Field label={d.conditionId} value={form.market.condition_id} onChange={(value) => setMarket("condition_id", value)} />
          <Field label={d.polymarketUrl} value={form.market.polymarket_url ?? ""} onChange={(value) => setMarket("polymarket_url", value)} />
          <Field className="md:col-span-2" label={d.marketQuestion} value={form.market.question} onChange={(value) => setMarket("question", value)} />
          <Field label={d.yesToken} value={form.market.yes_token_id} onChange={(value) => setMarket("yes_token_id", value)} />
          <Field label={d.noToken} value={form.market.no_token_id} onChange={(value) => setMarket("no_token_id", value)} />
          <Field label={d.rewardMinimumSize} value={form.version.reward_minimum_size} onChange={(value) => setVersion("reward_minimum_size", value)} />
          <Field label={d.rewardMaximumSpread} value={form.version.reward_maximum_spread} onChange={(value) => setVersion("reward_maximum_spread", value)} />
          <Field label={d.rewardDailyRate} value={form.version.reward_daily_rate ?? ""} onChange={(value) => setVersion("reward_daily_rate", value)} />
          <Field label={d.operatorNote} value={form.operator_note ?? ""} onChange={(value) => setForm((current) => ({ ...current, operator_note: value }))} />
        </CardContent>
      </Card> : null}

      {writable ? <Card>
        <CardHeader className="flex-row items-center justify-between">
          <CardTitle>{d.slots}</CardTitle>
          <Button
            variant="outline"
            onClick={() =>
              setVersion("quote_slots", [
                ...form.version.quote_slots,
                blankSlot(form.version.quote_slots.length + 1),
              ])
            }
          >
            {d.addSlot}
          </Button>
        </CardHeader>
        <CardContent className="space-y-4">
          {form.version.quote_slots.map((slot, index) => (
            <div key={index} className="space-y-3 rounded-lg border p-4">
              <div className="grid gap-3 md:grid-cols-4 xl:grid-cols-7">
                <Field label={d.slotKey} value={slot.slot_key} onChange={(value) => updateSlot(index, { slot_key: value })} />
                <SelectField label={d.outcome} value={slot.outcome} onChange={(value) => updateSlot(index, { outcome: value as "yes" | "no" })} options={["yes", "no"]} />
                <Field label={d.quantity} value={slot.quantity} onChange={(value) => updateSlot(index, { quantity: value })} />
                <SelectField label={d.pricing} value={slot.pricing_mode} onChange={(value) => setPricingMode(index, value as QuotePricingMode)} options={["book_rank", "fixed"]} />
                {slot.pricing_mode === "book_rank" ? (
                  <Field label={d.rank} type="number" value={String(slot.book_rank ?? 1)} onChange={(value) => updateSlot(index, { book_rank: Number(value) })} />
                ) : (
                  <Field label={d.fixed} value={slot.fixed_price ?? ""} onChange={(value) => updateSlot(index, { fixed_price: value })} />
                )}
                <Field label={d.offset} value={slot.price_offset} onChange={(value) => updateSlot(index, { price_offset: value })} />
                <Button
                  variant="ghost"
                  className="self-end"
                  disabled={form.version.quote_slots.length === 1}
                  onClick={() => setVersion("quote_slots", form.version.quote_slots.filter((_, slotIndex) => slotIndex !== index))}
                >
                  {d.removeSlot}
                </Button>
              </div>
              <div className="grid gap-3 md:grid-cols-4">
                <Field label={d.min} value={slot.minimum_price} onChange={(value) => updateSlot(index, { minimum_price: value })} />
                <Field label={d.max} value={slot.maximum_price} onChange={(value) => updateSlot(index, { maximum_price: value })} />
                <CheckField label={d.postOnly} checked={slot.post_only} onChange={(checked) => updateSlot(index, { post_only: checked })} />
                <CheckField label={d.enabled} checked={slot.enabled} onChange={(checked) => updateSlot(index, { enabled: checked })} />
              </div>
            </div>
          ))}
        </CardContent>
      </Card> : null}

      {writable ? <Card>
        <CardHeader>
          <CardTitle>{d.execution}</CardTitle>
        </CardHeader>
        <CardContent className="grid gap-4 md:grid-cols-3">
          <Field label={d.bookFreshness} type="number" value={String(form.version.book_freshness_ms)} onChange={(value) => setVersion("book_freshness_ms", Number(value))} />
          <Field label={d.downwardConfirm} type="number" value={String(form.version.downward_reprice_confirm_ms)} onChange={(value) => setVersion("downward_reprice_confirm_ms", Number(value))} />
          <Field label={d.upwardConfirm} type="number" value={String(form.version.upward_reprice_confirm_ms)} onChange={(value) => setVersion("upward_reprice_confirm_ms", Number(value))} />
          <Field label={d.repriceCooldown} type="number" value={String(form.version.reprice_cooldown_ms)} onChange={(value) => setVersion("reprice_cooldown_ms", Number(value))} />
          <Field label={d.maxReplaces} type="number" value={String(form.version.max_replaces_per_cycle)} onChange={(value) => setVersion("max_replaces_per_cycle", Number(value))} />
          <Field label={d.wallets} value={walletIds} onChange={setWalletIds} placeholder={d.walletIdsPlaceholder} />
          <Button className="self-end" disabled={isPending} onClick={submit}>
            {isPending ? dictionary.common.submitting : d.save}
          </Button>
        </CardContent>
      </Card> : null}

      <Card>
        <CardHeader>
          <CardTitle>{d.configured}</CardTitle>
        </CardHeader>
        <CardContent className="space-y-3">
          {loadError ? <p className="text-sm text-destructive">{loadError}</p> : null}
          {strategies.length === 0 && !loadError ? <p className="text-sm text-muted-foreground">{d.empty}</p> : null}
          {strategies.map((item) => (
            <div key={item.strategy.id} className="flex flex-wrap items-center justify-between gap-4 rounded-lg border p-4">
              <div>
                <p className="font-medium">{item.strategy.name}</p>
                <p className="text-xs text-muted-foreground">#{item.strategy.id} · {item.market.question}</p>
                <p className="text-xs text-muted-foreground">{item.strategy.owner_display_name} · {item.strategy.visibility} · 至 {new Date(item.strategy.active_until).toLocaleString()}</p>
              </div>
              <div className="flex flex-wrap gap-2">
                <StatusPill>{translateEnum(item.strategy.status)}</StatusPill>
                <StatusPill tone="primary">v{item.version.version_number}</StatusPill>
                <StatusPill>{item.quote_slots.length} {d.slotCount}</StatusPill>
                <StatusPill>{item.current_user_subscription?.wallets.length ?? 0} {d.walletCount}</StatusPill>
              </div>
            </div>
          ))}
        </CardContent>
      </Card>
    </div>
  );
}

function parseIds(value: string): number[] {
  return [...new Set(value.split(",").map((item) => Number(item.trim())).filter((id) => Number.isSafeInteger(id) && id > 0))];
}
function toLocalInput(value: string): string { const date = new Date(value); return new Date(date.getTime() - date.getTimezoneOffset() * 60_000).toISOString().slice(0, 16); }

function Field({ label, value, onChange, type = "text", placeholder, className }: { label: string; value: string; onChange: (value: string) => void; type?: string; placeholder?: string; className?: string }) {
  return <label className={`space-y-2 text-sm ${className ?? ""}`}><span>{label}</span><Input type={type} value={value} placeholder={placeholder} onChange={(event) => onChange(event.target.value)} /></label>;
}

function SelectField({ label, value, onChange, options }: { label: string; value: string; onChange: (value: string) => void; options: string[] }) {
  return <label className="space-y-2 text-sm"><span>{label}</span><select className="h-9 w-full rounded-md border border-input bg-background px-3 text-sm" value={value} onChange={(event) => onChange(event.target.value)}>{options.map((option) => <option key={option} value={option}>{translateEnum(option)}</option>)}</select></label>;
}

function CheckField({ label, checked, onChange }: { label: string; checked: boolean; onChange: (checked: boolean) => void }) {
  return <label className="flex items-center gap-3 self-end rounded-md border p-3 text-sm"><input type="checkbox" checked={checked} onChange={(event) => onChange(event.target.checked)} /><span>{label}</span></label>;
}
