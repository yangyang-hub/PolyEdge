import "server-only";

import { format, parseISO } from "date-fns";

export type Tone = "neutral" | "primary" | "success" | "warning" | "danger" | "violet";
export type AccentTone = "primary" | "success" | "danger" | "violet";

function toNumber(value: string): number {
  return Number.parseFloat(value);
}

export function humanizeSnakeCase(value: string): string {
  return value.replaceAll("_", " ");
}

export function uppercaseEnum(value: string): string {
  return value.toUpperCase().replaceAll("_", " ");
}

export function formatClock(value: string): string {
  return format(parseISO(value), "HH:mm:ss");
}

export function formatPercentFromRatio(value: string, digits = 0): string {
  return `${(toNumber(value) * 100).toFixed(digits)}%`;
}

export function formatSignedFixed(value: string, digits = 2): string {
  const numericValue = toNumber(value);
  return `${numericValue > 0 ? "+" : ""}${numericValue.toFixed(digits)}`;
}

export function formatCurrency(value: string): string {
  const numericValue = toNumber(value);

  return new Intl.NumberFormat("en-US", {
    style: "currency",
    currency: "USD",
    minimumFractionDigits: Number.isInteger(numericValue) ? 0 : 2,
    maximumFractionDigits: 2,
  }).format(numericValue);
}

export function formatInteger(value: string): string {
  return new Intl.NumberFormat("en-US").format(toNumber(value));
}

export function formatBucketWidth(value: string): string {
  return formatPercentFromRatio(value);
}

export function marketTradabilityTone(
  value: "tradable" | "manual_review" | "observe_only" | "blocked",
): Tone {
  if (value === "tradable") {
    return "success";
  }

  if (value === "manual_review") {
    return "warning";
  }

  if (value === "observe_only") {
    return "primary";
  }

  return "danger";
}

export function ambiguityTone(value: "low" | "medium" | "high"): Tone {
  if (value === "low") {
    return "success";
  }

  if (value === "medium") {
    return "warning";
  }

  return "danger";
}

export function eventStatusTone(value: "active" | "expired" | "invalidated" | "superseded"): Tone {
  if (value === "active") {
    return "success";
  }

  if (value === "superseded") {
    return "warning";
  }

  if (value === "invalidated") {
    return "danger";
  }

  return "neutral";
}

export function signalStateTone(
  value: "new" | "active" | "weakened" | "executed" | "invalidated" | "reversed" | "expired",
): Tone {
  if (value === "active") {
    return "success";
  }

  if (value === "new") {
    return "violet";
  }

  if (value === "executed") {
    return "primary";
  }

  if (value === "invalidated") {
    return "danger";
  }

  if (value === "reversed") {
    return "warning";
  }

  return "neutral";
}

export function approvalSeverityTone(value: "info" | "warning" | "critical"): Tone {
  if (value === "critical") {
    return "danger";
  }

  if (value === "warning") {
    return "warning";
  }

  return "primary";
}

export function alertSeverityTone(value: "warning" | "critical"): Tone {
  return value === "critical" ? "danger" : "warning";
}

export function alertStatusTone(value: "unresolved" | "watching" | "contained"): Tone {
  if (value === "unresolved") {
    return "danger";
  }

  if (value === "watching") {
    return "warning";
  }

  return "success";
}

export function bucketTone(value: "healthy" | "watch" | "breach"): Tone {
  if (value === "healthy") {
    return "success";
  }

  if (value === "watch") {
    return "primary";
  }

  return "danger";
}

export function metricToneForPnl(value: string): AccentTone {
  return toNumber(value) >= 0 ? "success" : "danger";
}
