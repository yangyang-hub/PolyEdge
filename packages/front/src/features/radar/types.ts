import type {
  ArbitrageOpportunityStatus,
  ArbitrageOpportunityType,
  ArbitrageValidationStatus,
} from "@/lib/contracts/dto";
import type { AccentTone, Tone } from "@/lib/formatters";

export type RadarFilter = "all" | "binary_buy_both" | "binary_sell_both";
export type RadarView = "active" | "validated" | "rejected" | "history";

export type RadarOpportunityItem = {
  id: string;
  marketId: string;
  marketQuestion: string;
  contextLabel: string;
  opportunityType: ArbitrageOpportunityType;
  typeLabel: string;
  typeTone: Tone;
  status: ArbitrageOpportunityStatus;
  statusLabel: string;
  statusTone: Tone;
  grossEdge: string;
  grossEdgeValue: number;
  priceSum: string;
  capacity: string;
  observedAt: string;
  observedClock: string;
  yesPrice: string;
  noPrice: string;
  yesSize: string;
  noSize: string;
  reasonCodes: string[];
  formula: string;
  validationStatus: ArbitrageValidationStatus | "unvalidated";
  validationLabel: string;
  validationTone: Tone;
  netEdge: string;
  netEdgeValue: number;
  feeEstimate: string;
  slippageBuffer: string;
  validatedCapacity: string;
  bookAge: string;
  bookAgeMs: number | null;
  validationReasonCodes: string[];
  candidateStatus: "candidate" | "watch" | "blocked";
  candidateLabel: string;
  candidateTone: Tone;
  candidateReason: string;
  isSelected: boolean;
};

export type RadarScanRow = {
  id: string;
  startedClock: string;
  finishedClock: string;
  marketCount: string;
  snapshotCount: string;
  opportunityCount: string;
  scannerVersion: string;
};

export type RadarTypeCount = {
  typeLabel: string;
  count: string;
  tone: Tone;
};

export type RadarTopMarket = {
  marketId: string;
  marketQuestion: string;
  opportunityCount: string;
  maxGrossEdge: string;
  avgGrossEdge: string;
  maxCapacity: string;
  duration: string;
};

export type RadarAnalysis = {
  generatedClock: string;
  lookbackHours: string;
  opportunityCount: string;
  marketCount: string;
  typeCounts: RadarTypeCount[];
  topMarkets: RadarTopMarket[];
};

export type RadarMetric = {
  title: string;
  value: string;
  hint: string;
  accent: AccentTone;
};

export type RadarPageData = {
  selectedOpportunityId: string;
  metrics: RadarMetric[];
  opportunities: RadarOpportunityItem[];
  scans: RadarScanRow[];
  analysis: RadarAnalysis | null;
};
