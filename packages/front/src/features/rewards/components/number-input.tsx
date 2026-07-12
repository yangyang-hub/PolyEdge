"use client";

import { useEffect, useId, useRef, useState } from "react";

import { Input } from "@/components/ui/input";
import type { DecimalValue } from "@/lib/contracts/dto";
import { toFiniteNumber } from "@/lib/formatters";
import { Hint } from "./rewards-config-fields";

function displayValue(value: DecimalValue): string {
  return String(toFiniteNumber(value));
}

export function NumberInput({
  label,
  value,
  suffix,
  hint,
  onChange,
}: {
  label: string;
  value: DecimalValue;
  suffix?: string;
  hint?: string;
  onChange: (value: string) => void;
}) {
  const id = useId();
  const focusedRef = useRef(false);
  const numericValue = displayValue(value);
  const [inputValue, setInputValue] = useState(() => numericValue);

  useEffect(() => {
    if (!focusedRef.current) setInputValue(numericValue);
  }, [numericValue]);

  function restoreOrCommit(rawValue: string) {
    const normalized = rawValue.trim();
    const parsed = Number(normalized);
    if (!normalized || !Number.isFinite(parsed) || parsed < 0) {
      setInputValue(numericValue);
      return;
    }
    const next = String(parsed);
    setInputValue(next);
    onChange(next);
  }

  return (
    <div className="space-y-1.5">
      <div className="flex min-h-6 items-center gap-1 text-xs font-medium text-muted-foreground">
        <label htmlFor={id}>{label}</label>
        {hint ? <Hint content={hint} /> : null}
      </div>
      <div className="flex">
        <Input
          id={id}
          name={id}
          type="text"
          inputMode="decimal"
          autoComplete="off"
          className="rounded-r-none font-mono"
          value={inputValue}
          aria-invalid={inputValue.trim() !== "" && (!Number.isFinite(Number(inputValue)) || Number(inputValue) < 0)}
          onFocus={() => {
            focusedRef.current = true;
          }}
          onChange={(event) => {
            const next = event.target.value;
            setInputValue(next);
            const parsed = Number(next);
            if (next.trim() !== "" && Number.isFinite(parsed) && parsed >= 0) onChange(next);
          }}
          onBlur={() => {
            focusedRef.current = false;
            restoreOrCommit(inputValue);
          }}
          onKeyDown={(event) => {
            if (event.key === "Enter") event.currentTarget.blur();
            if (event.key === "Escape") {
              setInputValue(numericValue);
              event.currentTarget.blur();
            }
          }}
        />
        {suffix ? (
          <span className="flex h-8 min-w-8 items-center justify-center rounded-r-lg border border-l-0 border-input px-2 text-xs text-muted-foreground">
            {suffix}
          </span>
        ) : null}
      </div>
    </div>
  );
}
