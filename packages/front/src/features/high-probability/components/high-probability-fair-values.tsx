import type { HighProbabilityFairValueDto } from "@/lib/contracts/dto";
import { StatusPill } from "@/components/shared/status-pill";
import { Card, CardContent, CardDescription, CardHeader, CardTitle } from "@/components/ui/card";
import { Table, TableBody, TableCell, TableHead, TableHeader, TableRow } from "@/components/ui/table";
import {
  fairValueBand,
  fairValueEligibleTone,
  fairValueFallbackLabel,
  fairValueSideLabel,
  formatOptionalProbability,
  formatProbability,
} from "@/features/high-probability/lib/high-probability-formatters";
import { formatInteger, formatOptionalClock } from "@/lib/formatters";
import { dictionary } from "@/lib/i18n/dictionaries";

const FALLBACK_WARN_LEVEL = 4;

/**
 * Read-only fair value diagnostics table. Renders the conservative
 * `fair_yes_low/mid/high` band, confidence, uncertainty, the bucket it was
 * derived from (with its coarseness fallback level) and the reason codes that
 * decide whether the estimate is live-eligible for the Rewards market maker.
 */
export function HighProbabilityFairValuesTable({
  fairValues,
}: {
  fairValues: HighProbabilityFairValueDto[];
}) {
  const t = dictionary.highProbability;

  return (
    <Card>
      <CardHeader>
        <CardTitle>{t.fairValues}</CardTitle>
        <CardDescription>{t.fairValuesDescription}</CardDescription>
      </CardHeader>
      <CardContent>
        <Table className="min-w-[1080px] table-fixed">
          <TableHeader>
            <TableRow>
              <TableHead>{t.fairValueEligible}</TableHead>
              <TableHead className="w-[220px]">{t.fairValueCondition}</TableHead>
              <TableHead>{t.fairValueSide}</TableHead>
              <TableHead>{t.fairValuePrice}</TableHead>
              <TableHead>{t.fairValueBand}</TableHead>
              <TableHead>{t.fairValueMid}</TableHead>
              <TableHead>{t.fairValueMarketImplied}</TableHead>
              <TableHead>{t.fairValueBaseRate}</TableHead>
              <TableHead>{t.fairValueConfidence}</TableHead>
              <TableHead>{t.fairValueUncertainty}</TableHead>
              <TableHead>{t.fairValueSamples}</TableHead>
              <TableHead>{t.fairValueFallback}</TableHead>
              <TableHead className="w-[240px]">{t.fairValueReasons}</TableHead>
              <TableHead>{t.fairValueExpires}</TableHead>
            </TableRow>
          </TableHeader>
          <TableBody>
            {fairValues.length === 0 ? (
              <TableRow>
                <TableCell colSpan={14} className="py-8 text-center text-sm text-muted-foreground">
                  {t.noFairValues}
                </TableCell>
              </TableRow>
            ) : (
              fairValues.map((fairValue) => (
                <TableRow key={`${fairValue.condition_id}:${fairValue.model_version}`}>
                  <TableCell className="align-top">
                    <StatusPill tone={fairValueEligibleTone(fairValue.live_eligible)}>
                      {fairValue.live_eligible ? dictionary.common.yes : dictionary.common.no}
                    </StatusPill>
                  </TableCell>
                  <TableCell className="whitespace-normal align-top font-mono text-xs">
                    {fairValue.condition_id}
                  </TableCell>
                  <TableCell className="align-top">
                    <StatusPill tone={fairValue.side_used === "yes" ? "primary" : "neutral"}>
                      {fairValueSideLabel(fairValue.side_used)}
                    </StatusPill>
                  </TableCell>
                  <TableCell className="align-top font-mono">
                    {formatProbability(fairValue.price_used)}
                  </TableCell>
                  <TableCell className="align-top font-mono text-xs text-muted-foreground">
                    {fairValueBand(fairValue)}
                  </TableCell>
                  <TableCell className="align-top font-mono">
                    {formatProbability(fairValue.fair_yes_mid)}
                  </TableCell>
                  <TableCell className="align-top font-mono">
                    {formatOptionalProbability(fairValue.market_implied)}
                  </TableCell>
                  <TableCell className="align-top font-mono">
                    {formatOptionalProbability(fairValue.base_rate)}
                  </TableCell>
                  <TableCell className="align-top font-mono">
                    {formatProbability(fairValue.confidence)}
                  </TableCell>
                  <TableCell className="align-top font-mono">
                    {formatInteger(Math.round(Number(fairValue.uncertainty_cents)))}c
                  </TableCell>
                  <TableCell className="align-top font-mono">
                    {formatInteger(fairValue.sample_count)}
                  </TableCell>
                  <TableCell className="align-top">
                    <StatusPill tone={fairValue.fallback_level >= FALLBACK_WARN_LEVEL ? "warning" : "neutral"}>
                      {fairValueFallbackLabel(fairValue.fallback_level)}
                    </StatusPill>
                  </TableCell>
                  <TableCell className="whitespace-normal align-top text-xs text-muted-foreground">
                    {fairValue.reason_codes.length > 0
                      ? fairValue.reason_codes.join(" / ")
                      : dictionary.common.none}
                  </TableCell>
                  <TableCell className="align-top font-mono text-xs text-muted-foreground">
                    {formatOptionalClock(fairValue.expires_at)}
                  </TableCell>
                </TableRow>
              ))
            )}
          </TableBody>
        </Table>
      </CardContent>
    </Card>
  );
}
