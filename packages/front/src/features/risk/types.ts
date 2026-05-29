import type { getRiskPageData } from "@/features/risk/loaders/risk-page-data";

export type RiskPageData = Awaited<ReturnType<typeof getRiskPageData>>;
export type RiskDialog = "release" | "kill_switch" | null;
export type RiskAlertFilter = "all" | "unresolved" | "watching";
