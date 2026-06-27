export type Tone = "neutral" | "primary" | "success" | "warning" | "danger" | "violet";
export type AccentTone = "primary" | "success" | "danger" | "violet";
export type NumericValue = number | string | null | undefined;

export function toFiniteNumber(value: NumericValue): number {
  if (value === null || value === undefined) {
    return 0;
  }

  const numericValue = typeof value === "number" ? value : Number.parseFloat(value);
  return Number.isFinite(numericValue) ? numericValue : 0;
}

export function humanizeSnakeCase(value: string): string {
  return value.replaceAll("_", " ");
}

export function uppercaseEnum(value: string): string {
  return value.toUpperCase().replaceAll("_", " ");
}

export function formatClock(value: string): string {
  return new Intl.DateTimeFormat("en-GB", {
    hour: "2-digit",
    minute: "2-digit",
    second: "2-digit",
    hour12: false,
  }).format(new Date(value));
}

export function formatOptionalClock(value: string | null | undefined, fallback = "n/a"): string {
  if (!value) {
    return fallback;
  }

  const date = new Date(value);
  if (Number.isNaN(date.getTime())) {
    return fallback;
  }

  return new Intl.DateTimeFormat("en-GB", {
    hour: "2-digit",
    minute: "2-digit",
    second: "2-digit",
    hour12: false,
  }).format(date);
}

export function formatFixed(value: NumericValue, digits = 2): string {
  return toFiniteNumber(value).toFixed(digits);
}

export function formatPercentFromRatio(value: NumericValue, digits = 0): string {
  return `${(toFiniteNumber(value) * 100).toFixed(digits)}%`;
}

export function formatSignedPercent(value: NumericValue, digits = 1): string {
  const numericValue = toFiniteNumber(value);
  return `${numericValue > 0 ? "+" : ""}${(numericValue * 100).toFixed(digits)}%`;
}

export function formatSignedFixed(value: NumericValue, digits = 2): string {
  const numericValue = toFiniteNumber(value);
  return `${numericValue > 0 ? "+" : ""}${numericValue.toFixed(digits)}`;
}

export function formatCurrency(value: NumericValue): string {
  const numericValue = toFiniteNumber(value);

  return new Intl.NumberFormat("en-US", {
    style: "currency",
    currency: "USD",
    minimumFractionDigits: Number.isInteger(numericValue) ? 0 : 2,
    maximumFractionDigits: 2,
  }).format(numericValue);
}

export function formatUsdFixed(value: NumericValue, digits = 2): string {
  return new Intl.NumberFormat("en-US", {
    style: "currency",
    currency: "USD",
    minimumFractionDigits: digits,
    maximumFractionDigits: digits,
  }).format(toFiniteNumber(value));
}

export function formatInteger(value: NumericValue): string {
  return new Intl.NumberFormat("en-US").format(toFiniteNumber(value));
}

export function formatBucketWidth(value: NumericValue): string {
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

export function metricToneForPnl(value: NumericValue): AccentTone {
  return toFiniteNumber(value) >= 0 ? "success" : "danger";
}

export function approvalRiskPercent(value: "info" | "warning" | "critical"): string {
  if (value === "critical") {
    return "98%";
  }

  if (value === "warning") {
    return "32%";
  }

  return "08%";
}
