import assert from "node:assert/strict";
import test from "node:test";

import {
  calculateValidationSummary,
  deriveCandidatePreview,
} from "./radar-state";

test("deriveCandidatePreview blocks expired opportunities even after valid validation", () => {
  const preview = deriveCandidatePreview({
    opportunityStatus: "expired",
    validationStatus: "valid",
    hasValidation: true,
    netEdgeValue: 0.02,
  });

  assert.equal(preview.status, "blocked");
  assert.equal(preview.tone, "neutral");
  assert.equal(preview.reason, "expired opportunity");
});

test("deriveCandidatePreview keeps unvalidated active opportunities on watch", () => {
  const preview = deriveCandidatePreview({
    opportunityStatus: "observed",
    validationStatus: "unvalidated",
    hasValidation: false,
    netEdgeValue: 0,
  });

  assert.equal(preview.status, "watch");
  assert.equal(preview.reason, "waiting for validation");
});

test("deriveCandidatePreview accepts only valid positive-net opportunities as candidates", () => {
  assert.equal(
    deriveCandidatePreview({
      opportunityStatus: "observed",
      validationStatus: "valid",
      hasValidation: true,
      netEdgeValue: 0.01,
    }).status,
    "candidate",
  );
  assert.equal(
    deriveCandidatePreview({
      opportunityStatus: "observed",
      validationStatus: "valid",
      hasValidation: true,
      netEdgeValue: 0,
    }).reason,
    "non-positive net edge",
  );
  assert.equal(
    deriveCandidatePreview({
      opportunityStatus: "observed",
      validationStatus: "price_moved",
      hasValidation: true,
      netEdgeValue: 0.01,
    }).reason,
    "price moved",
  );
});

test("calculateValidationSummary excludes unvalidated opportunities from pass-rate denominator", () => {
  const summary = calculateValidationSummary([
    { validationStatus: "valid" },
    { validationStatus: "price_moved" },
    { validationStatus: "unvalidated" },
  ]);

  assert.equal(summary.validCount, 1);
  assert.equal(summary.rejectedCount, 1);
  assert.equal(summary.completedValidationCount, 2);
  assert.equal(summary.passRate, 0.5);
});
