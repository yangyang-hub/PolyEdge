"use client";

import { startTransition, useMemo, useState } from "react";
import { Ban, Eye, ShieldCheck, UserCheck } from "lucide-react";
import { toast } from "sonner";

import { OperationFeedbackBanner } from "@/components/shared/operation-feedback-banner";
import { PaginationBar } from "@/components/pagination-bar";
import { StatusPill } from "@/components/shared/status-pill";
import { TruncateText } from "@/components/shared/truncate-text";
import { Button } from "@/components/ui/button";
import { Card, CardContent, CardDescription, CardHeader, CardTitle } from "@/components/ui/card";
import { usePagination } from "@/hooks/use-pagination";
import {
  updateSmartWalletCandidateStatusAction,
  type SmartMoneyActionResult,
} from "@/lib/api/actions";
import type {
  SmartMoneySnapshotDto,
  SmartWalletCandidateDto,
  SmartWalletCandidateStatus,
  SmartWalletTier,
} from "@/lib/contracts/dto";
import { formatShortAddress } from "@/lib/format-address";
import {
  formatInteger,
  formatOptionalClock,
  formatPercentFromRatio,
  formatUsdFixed,
  uppercaseEnum,
  type Tone,
} from "@/lib/formatters";
import { dictionary } from "@/lib/i18n/dictionaries";

function candidateStatusTone(status: SmartWalletCandidateStatus): Tone {
  if (status === "tracked") {
    return "success";
  }

  if (status === "watch") {
    return "primary";
  }

  if (status === "blocked" || status === "rejected") {
    return "danger";
  }

  return "neutral";
}

function walletTierTone(tier: SmartWalletTier): Tone {
  if (tier === "approved") {
    return "success";
  }

  if (tier === "watch") {
    return "primary";
  }

  if (tier === "blocked") {
    return "danger";
  }

  return "neutral";
}

function actionButtons(candidate: SmartWalletCandidateDto) {
  return [
    { status: "watch" as const, icon: Eye, label: dictionary.copytrade.smart.watch },
    { status: "tracked" as const, icon: UserCheck, label: dictionary.copytrade.smart.track },
    { status: "blocked" as const, icon: ShieldCheck, label: dictionary.copytrade.smart.block },
    { status: "rejected" as const, icon: Ban, label: dictionary.copytrade.smart.reject },
  ].filter((action) => action.status !== candidate.status);
}

export function SmartMoneyCandidatesPanel({
  snapshot,
  onSnapshotChange,
}: {
  snapshot: SmartMoneySnapshotDto;
  onSnapshotChange: (snapshot: SmartMoneySnapshotDto) => void;
}) {
  const t = dictionary.copytrade.smart;
  const [feedback, setFeedback] = useState<SmartMoneyActionResult | null>(null);
  const [pendingWallet, setPendingWallet] = useState<string | null>(null);
  const pagination = usePagination(snapshot.candidates.length, 12);

  const profilesByWallet = useMemo(
    () =>
      new Map(snapshot.profiles.map((profile) => [profile.wallet_address.toLowerCase(), profile])),
    [snapshot.profiles],
  );
  const scoresByWallet = useMemo(
    () => new Map(snapshot.scores.map((score) => [score.wallet_address.toLowerCase(), score])),
    [snapshot.scores],
  );

  function applyResult(result: SmartMoneyActionResult) {
    setFeedback(result);
    if (result.ok) {
      toast.success(result.message);
    } else {
      toast.error(result.message);
    }
    if (result.snapshot) {
      onSnapshotChange(result.snapshot);
    }
  }

  function setCandidateStatus(candidate: SmartWalletCandidateDto, status: SmartWalletCandidateStatus) {
    setPendingWallet(candidate.wallet_address);
    startTransition(() => {
      void updateSmartWalletCandidateStatusAction({
        walletAddress: candidate.wallet_address,
        source: candidate.source,
        status,
        reason: t.manualStatusReason,
      })
        .then(applyResult)
        .finally(() => setPendingWallet(null));
    });
  }

  return (
    <Card>
      <CardHeader className="flex flex-col gap-4 border-b border-border/70 xl:flex-row xl:items-center xl:justify-between">
        <div>
          <CardTitle className="font-heading text-base">{t.title}</CardTitle>
          <CardDescription>{t.description}</CardDescription>
        </div>
        <div className="grid grid-cols-2 gap-2 text-xs text-muted-foreground sm:grid-cols-4">
          <span>
            {t.candidates}: {formatInteger(snapshot.status.candidates)}
          </span>
          <span>
            {t.watch}: {formatInteger(snapshot.status.watch_wallets)}
          </span>
          <span>
            {t.tracked}: {formatInteger(snapshot.status.tracked_wallets)}
          </span>
          <span>
            {t.lastTrade}: {formatOptionalClock(snapshot.status.last_trade_at, dictionary.common.none)}
          </span>
        </div>
      </CardHeader>
      <CardContent className="space-y-4">
        {feedback ? <OperationFeedbackBanner feedback={feedback} onDismiss={() => setFeedback(null)} /> : null}

        {snapshot.candidates.length === 0 ? (
          <p className="py-8 text-center text-sm text-muted-foreground">{t.noCandidates}</p>
        ) : (
          <div className="overflow-auto">
            <table className="w-full min-w-[920px] text-xs">
              <thead>
                <tr className="border-b border-border/60 text-left text-muted-foreground">
                  <th className="pb-2 pr-3">{t.wallet}</th>
                  <th className="pb-2 pr-3">{t.source}</th>
                  <th className="pb-2 pr-3">{t.status}</th>
                  <th className="pb-2 pr-3">{t.score}</th>
                  <th className="pb-2 pr-3">{t.profile}</th>
                  <th className="pb-2 pr-3">{t.seen}</th>
                  <th className="pb-2 pr-3 text-right">{t.actions}</th>
                </tr>
              </thead>
              <tbody>
                {snapshot.candidates.slice(pagination.start, pagination.end).map((candidate) => {
                  const walletKey = candidate.wallet_address.toLowerCase();
                  const profile = profilesByWallet.get(walletKey);
                  const score = scoresByWallet.get(walletKey);
                  const pending = pendingWallet === candidate.wallet_address;

                  return (
                    <tr key={`${candidate.wallet_address}-${candidate.source}`} className="border-b border-border/20">
                      <td className="py-3 pr-3">
                        <p className="font-mono text-foreground">{formatShortAddress(candidate.wallet_address)}</p>
                        {candidate.reason ? (
                          <TruncateText
                            text={candidate.reason}
                            lines={1}
                            className="mt-1 block text-muted-foreground"
                          />
                        ) : null}
                      </td>
                      <td className="py-3 pr-3 text-muted-foreground">{candidate.source}</td>
                      <td className="py-3 pr-3">
                        <StatusPill tone={candidateStatusTone(candidate.status)}>
                          {t.statusLabels[candidate.status]}
                        </StatusPill>
                      </td>
                      <td className="py-3 pr-3">
                        {score ? (
                          <div className="space-y-1">
                            <p className="font-mono text-foreground">{formatPercentFromRatio(score.total_score, 1)}</p>
                            <StatusPill tone={walletTierTone(score.tier)}>{uppercaseEnum(score.tier)}</StatusPill>
                          </div>
                        ) : (
                          <span className="text-muted-foreground">{dictionary.common.pending}</span>
                        )}
                      </td>
                      <td className="py-3 pr-3 text-muted-foreground">
                        {profile ? (
                          <div className="space-y-1">
                            <p>
                              {formatInteger(profile.trade_count)} {t.trades} /{" "}
                              {formatUsdFixed(profile.total_volume_usd, 0)}
                            </p>
                            <p>
                              {t.winRate} {formatPercentFromRatio(profile.win_rate, 1)} / {t.roi}{" "}
                              {formatPercentFromRatio(profile.roi, 1)}
                            </p>
                          </div>
                        ) : (
                          dictionary.common.pending
                        )}
                      </td>
                      <td className="py-3 pr-3 text-muted-foreground">
                        <div className="space-y-1">
                          <p>{formatOptionalClock(candidate.last_seen_at)}</p>
                          <p>{formatOptionalClock(candidate.last_analyzed_at, t.notAnalyzed)}</p>
                        </div>
                      </td>
                      <td className="py-3 pr-3">
                        <div className="flex justify-end gap-1">
                          {actionButtons(candidate).map((action) => {
                            const Icon = action.icon;
                            return (
                              <Button
                                key={action.status}
                                size="sm"
                                variant={
                                  action.status === "blocked" || action.status === "rejected"
                                    ? "outline"
                                    : "default"
                                }
                                className="h-7 px-2 text-xs"
                                disabled={Boolean(pendingWallet) || pending}
                                onClick={() => setCandidateStatus(candidate, action.status)}
                              >
                                <Icon className="size-3" /> {action.label}
                              </Button>
                            );
                          })}
                        </div>
                      </td>
                    </tr>
                  );
                })}
              </tbody>
            </table>
            <PaginationBar
              pagination={pagination}
              totalItems={snapshot.candidates.length}
              className="mt-3 flex items-center justify-between border-t border-border/70 pt-3"
            />
          </div>
        )}
      </CardContent>
    </Card>
  );
}
