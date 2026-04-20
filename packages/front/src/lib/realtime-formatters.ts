export type RealtimeTone = "neutral" | "primary" | "success" | "warning" | "danger" | "violet";
export type MetricTone = "primary" | "success" | "danger" | "violet";
export type ApprovalSeverityTone = RealtimeTone;

function toNumber(value: number | string): number {
  return typeof value === "number" ? value : Number.parseFloat(value);
}

export function humanizeSnakeCase(value: string): string {
  return value.replaceAll("_", " ");
}

export function uppercaseEnum(value: string): string {
  return value.toUpperCase().replaceAll("_", " ");
}

export function formatPercentFromRatio(value: number | string, digits = 0): string {
  return `${(toNumber(value) * 100).toFixed(digits)}%`;
}

export function formatSignedFixed(value: number | string, digits = 2): string {
  const numericValue = toNumber(value);
  return `${numericValue > 0 ? "+" : ""}${numericValue.toFixed(digits)}`;
}

export function formatCurrency(value: number | string): string {
  const numericValue = toNumber(value);

  return new Intl.NumberFormat("en-US", {
    style: "currency",
    currency: "USD",
    minimumFractionDigits: Number.isInteger(numericValue) ? 0 : 2,
    maximumFractionDigits: 2,
  }).format(numericValue);
}

export function formatClock(value: string): string {
  return new Intl.DateTimeFormat("en-GB", {
    hour: "2-digit",
    minute: "2-digit",
    second: "2-digit",
    hour12: false,
  }).format(new Date(value));
}

export function signalStateTone(
  value: "new" | "active" | "weakened" | "executed" | "invalidated" | "reversed" | "expired",
): RealtimeTone {
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

export function alertSeverityTone(value: "warning" | "critical"): RealtimeTone {
  return value === "critical" ? "danger" : "warning";
}

export function alertStatusTone(value: "unresolved" | "watching" | "contained"): RealtimeTone {
  if (value === "unresolved") {
    return "danger";
  }

  if (value === "watching") {
    return "warning";
  }

  return "success";
}

export function approvalSeverityTone(value: "info" | "warning" | "critical"): ApprovalSeverityTone {
  if (value === "critical") {
    return "danger";
  }

  if (value === "warning") {
    return "warning";
  }

  return "primary";
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

export function metricToneForPnl(value: number | string): MetricTone {
  return toNumber(value) >= 0 ? "success" : "danger";
}
