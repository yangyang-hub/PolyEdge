export type RadarOpportunityStatus = "observed" | "expired" | "repeated";
export type RadarValidationStatus =
  | "unvalidated"
  | "valid"
  | "stale_book"
  | "insufficient_depth"
  | "price_moved"
  | "fees_exceed_edge"
  | "below_threshold"
  | "invalid_market"
  | "error";

export type RadarCandidateStatus = "candidate" | "watch" | "blocked";
export type RadarCandidateTone = "success" | "warning" | "neutral";

export type RadarCandidatePreview = {
  status: RadarCandidateStatus;
  label: string;
  tone: RadarCandidateTone;
  reason: string;
};

export type RadarValidationSummary = {
  validCount: number;
  rejectedCount: number;
  completedValidationCount: number;
  passRate: number;
};

function humanizeSnakeCase(value: string): string {
  return value.replaceAll("_", " ");
}

function candidateTone(status: RadarCandidateStatus): RadarCandidateTone {
  if (status === "candidate") {
    return "success";
  }

  if (status === "watch") {
    return "warning";
  }

  return "neutral";
}

export function deriveCandidatePreview(input: {
  opportunityStatus: RadarOpportunityStatus;
  validationStatus: RadarValidationStatus;
  hasValidation: boolean;
  netEdgeValue: number;
}): RadarCandidatePreview {
  if (input.opportunityStatus === "expired") {
    return {
      status: "blocked",
      label: "blocked",
      tone: candidateTone("blocked"),
      reason: "expired opportunity",
    };
  }

  if (!input.hasValidation) {
    return {
      status: "watch",
      label: "watch",
      tone: candidateTone("watch"),
      reason: "waiting for validation",
    };
  }

  if (input.validationStatus !== "valid") {
    return {
      status: "blocked",
      label: "blocked",
      tone: candidateTone("blocked"),
      reason: humanizeSnakeCase(input.validationStatus),
    };
  }

  if (input.netEdgeValue <= 0) {
    return {
      status: "blocked",
      label: "blocked",
      tone: candidateTone("blocked"),
      reason: "non-positive net edge",
    };
  }

  return {
    status: "candidate",
    label: "candidate",
    tone: candidateTone("candidate"),
    reason: "valid read-only candidate",
  };
}

export function calculateValidationSummary(
  opportunities: Array<{ validationStatus: RadarValidationStatus }>,
): RadarValidationSummary {
  const validCount = opportunities.filter((opportunity) => opportunity.validationStatus === "valid").length;
  const rejectedCount = opportunities.filter(
    (opportunity) =>
      opportunity.validationStatus !== "valid" &&
      opportunity.validationStatus !== "unvalidated",
  ).length;
  const completedValidationCount = validCount + rejectedCount;

  return {
    validCount,
    rejectedCount,
    completedValidationCount,
    passRate: completedValidationCount > 0 ? validCount / completedValidationCount : 0,
  };
}
