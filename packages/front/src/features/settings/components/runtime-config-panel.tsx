"use client";

import { startTransition, useMemo, useState } from "react";
import { Save } from "lucide-react";

import { OperationFeedbackBanner } from "@/components/shared/operation-feedback-banner";
import { StatusPill } from "@/components/shared/status-pill";
import { Button } from "@/components/ui/button";
import { Card, CardAction, CardContent, CardDescription, CardHeader, CardTitle } from "@/components/ui/card";
import { Input } from "@/components/ui/input";
import { Textarea } from "@/components/ui/textarea";
import type { RuntimeConfigEntryDto } from "@/lib/contracts/dto";
import { dictionary } from "@/lib/i18n/dictionaries";
import {
  updateRuntimeConfigAction,
  type RuntimeConfigActionResult,
} from "@/lib/api/actions";

type RuntimeConfigPanelProps = {
  entries: RuntimeConfigEntryDto[];
};

const sectionOrder = ["risk", "polymarket", "arbitrage", "rewards", "news", "worker"];

function sectionLabel(section: string): string {
  const labels: Record<string, string> = {
    risk: "Risk",
    polymarket: "Polymarket",
    arbitrage: "Arbitrage",
    rewards: "Rewards",
    news: "News",
    worker: "Worker",
  };

  return labels[section] ?? section;
}

export function RuntimeConfigPanel({ entries }: RuntimeConfigPanelProps) {
  const [configEntries, setConfigEntries] = useState(entries);
  const [values, setValues] = useState(() => Object.fromEntries(entries.map((entry) => [entry.key, entry.value])));
  const [feedback, setFeedback] = useState<RuntimeConfigActionResult | null>(null);
  const [pending, setPending] = useState(false);

  const groupedEntries = useMemo(() => {
    const groups = new Map<string, RuntimeConfigEntryDto[]>();
    for (const entry of configEntries) {
      const group = groups.get(entry.section) ?? [];
      group.push(entry);
      groups.set(entry.section, group);
    }

    return Array.from(groups.entries()).sort(
      ([left], [right]) => sectionOrder.indexOf(left) - sectionOrder.indexOf(right),
    );
  }, [configEntries]);

  function save() {
    setPending(true);
    startTransition(() => {
      void updateRuntimeConfigAction({ values })
        .then((result) => {
          setFeedback(result);
          if (result.entries) {
            setConfigEntries(result.entries);
            setValues(Object.fromEntries(result.entries.map((entry) => [entry.key, entry.value])));
          }
        })
        .finally(() => setPending(false));
    });
  }

  return (
    <Card className="md:col-span-2">
      <CardHeader>
        <CardTitle>{dictionary.settings.runtimeConfig}</CardTitle>
        <CardDescription>{dictionary.settings.runtimeConfigDescription}</CardDescription>
        <CardAction>
          <Button type="button" size="sm" disabled={pending} onClick={save}>
            <Save className="size-4" />
            {dictionary.settings.saveRuntimeConfig}
          </Button>
        </CardAction>
      </CardHeader>
      <CardContent className="space-y-5">
        {feedback ? <OperationFeedbackBanner feedback={feedback} /> : null}
        <div className="grid gap-4 xl:grid-cols-2">
          {groupedEntries.map(([section, groupEntries]) => (
            <section key={section} className="space-y-3">
              <div className="flex items-center justify-between gap-3">
                <h3 className="font-heading text-sm text-foreground">{sectionLabel(section)}</h3>
                <StatusPill tone="neutral">{groupEntries.length}</StatusPill>
              </div>
              <div className="grid gap-3 sm:grid-cols-2">
                {groupEntries.map((entry) => (
                  <ConfigField
                    key={entry.key}
                    entry={entry}
                    value={values[entry.key] ?? ""}
                    restartLabel={dictionary.settings.restartRequired}
                    enabledLabel={dictionary.common.enabled}
                    disabledLabel={dictionary.common.disabled}
                    onChange={(value) => setValues((current) => ({ ...current, [entry.key]: value }))}
                  />
                ))}
              </div>
            </section>
          ))}
        </div>
      </CardContent>
    </Card>
  );
}

function ConfigField({
  entry,
  value,
  restartLabel,
  enabledLabel,
  disabledLabel,
  onChange,
}: {
  entry: RuntimeConfigEntryDto;
  value: string;
  restartLabel: string;
  enabledLabel: string;
  disabledLabel: string;
  onChange: (value: string) => void;
}) {
  return (
    <label className={entry.value_type === "json" ? "space-y-1.5 sm:col-span-2" : "space-y-1.5"}>
      <span className="flex items-center justify-between gap-2 text-xs font-medium text-muted-foreground">
        <span>{entry.label}</span>
        {entry.restart_required ? <span className="font-normal">{restartLabel}</span> : null}
      </span>
      <ConfigControl
        entry={entry}
        value={value}
        enabledLabel={enabledLabel}
        disabledLabel={disabledLabel}
        onChange={onChange}
      />
      <span className="block truncate font-mono text-[11px] text-muted-foreground" title={entry.env_name}>
        {entry.env_name}
      </span>
    </label>
  );
}

function ConfigControl({
  entry,
  value,
  enabledLabel,
  disabledLabel,
  onChange,
}: {
  entry: RuntimeConfigEntryDto;
  value: string;
  enabledLabel: string;
  disabledLabel: string;
  onChange: (value: string) => void;
}) {
  if (entry.value_type === "boolean") {
    return (
      <label className="flex h-8 items-center gap-2 rounded-lg border border-input bg-background px-2.5 text-sm">
        <input
          type="checkbox"
          className="size-4 accent-primary"
          checked={value === "true" || value === "1"}
          onChange={(event) => onChange(event.target.checked ? "true" : "false")}
        />
        {value === "true" || value === "1" ? enabledLabel : disabledLabel}
      </label>
    );
  }

  if (entry.options.length > 0) {
    return (
      <select
        className="h-8 w-full rounded-lg border border-input bg-background px-2.5 text-sm"
        value={value}
        onChange={(event) => onChange(event.target.value)}
      >
        {entry.options.map((option) => (
          <option key={option} value={option}>
            {option}
          </option>
        ))}
      </select>
    );
  }

  if (entry.value_type === "json") {
    return (
      <Textarea
        className="min-h-28 font-mono text-xs"
        value={value}
        onChange={(event) => onChange(event.target.value)}
      />
    );
  }

  return (
    <Input
      type={entry.value_type === "integer" || entry.value_type === "decimal" ? "number" : "text"}
      className={entry.value_type === "integer" || entry.value_type === "decimal" ? "font-mono" : undefined}
      value={value}
      step={entry.value_type === "decimal" ? "0.001" : undefined}
      onChange={(event) => onChange(event.target.value)}
    />
  );
}
