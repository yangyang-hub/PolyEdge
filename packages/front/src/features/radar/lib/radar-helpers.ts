import type { RadarOpportunityItem, RadarView } from "../types";

export function viewMatches(view: RadarView, opportunity: RadarOpportunityItem): boolean {
  if (view === "active") {
    return opportunity.status !== "expired";
  }

  if (view === "validated") {
    return opportunity.validationStatus === "valid";
  }

  if (view === "rejected") {
    return opportunity.validationStatus !== "valid" && opportunity.validationStatus !== "unvalidated";
  }

  return true;
}

export function compareRadarPriority(left: RadarOpportunityItem, right: RadarOpportunityItem): number {
  const leftValid = left.validationStatus === "valid" ? 1 : 0;
  const rightValid = right.validationStatus === "valid" ? 1 : 0;
  const leftAge = left.bookAgeMs ?? Number.MAX_SAFE_INTEGER;
  const rightAge = right.bookAgeMs ?? Number.MAX_SAFE_INTEGER;

  return (
    rightValid - leftValid ||
    right.netEdgeValue - left.netEdgeValue ||
    leftAge - rightAge ||
    Date.parse(right.observedAt) - Date.parse(left.observedAt)
  );
}
