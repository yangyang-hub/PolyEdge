import { Clipboard, ExternalLink } from "lucide-react";

import { Button } from "@/components/ui/button";
import { Card, CardContent, CardDescription, CardHeader, CardTitle } from "@/components/ui/card";
import {
  buildPolygonscanTokenUrl,
  buildPolygonscanTxUrl,
  polygonChain,
  type PolygonFundingToken,
} from "@/features/funding/lib/polygon-funding";
import type { FundingSubmissionSnapshot } from "@/features/funding/types";
import { formatShortAddress } from "@/lib/format-address";
import type { FundingStatusDto } from "@/lib/contracts/dto";
import { dictionary } from "@/lib/i18n/dictionaries";

type FundingReviewCardProps = {
  amount: string;
  amountUnits: bigint | null;
  onCopy: (value: string, message: string) => void;
  selectedToken: PolygonFundingToken;
  status: FundingStatusDto;
  submission: FundingSubmissionSnapshot;
  tokenNote: string;
};

export function FundingReviewCard({
  amount,
  amountUnits,
  onCopy,
  selectedToken,
  status,
  submission,
  tokenNote,
}: FundingReviewCardProps) {
  const sourceAddress = status.source_address;
  const polymarketWalletAddress = status.polymarket_wallet_address;
  const bridgeDepositAddress = submission.transfer?.bridge_deposit_address ?? null;

  return (
    <Card>
      <CardHeader>
        <CardTitle>{dictionary.funding.reviewTitle}</CardTitle>
        <CardDescription>{dictionary.funding.reviewDescription}</CardDescription>
      </CardHeader>
      <CardContent className="space-y-3">
        <ReviewRow label={dictionary.funding.network} value={`${polygonChain.chainName} (${polygonChain.chainId})`} />
        <ReviewRow label={dictionary.funding.token} value={`${selectedToken.symbol} - ${tokenNote}`} />
        <ReviewRow
          label={dictionary.funding.tokenContract}
          value={formatShortAddress(selectedToken.address)}
          action={
            <>
              <IconButton
                label={dictionary.funding.copy}
                onClick={() => onCopy(selectedToken.address, dictionary.funding.copiedToken)}
              >
                <Clipboard className="size-3.5" />
              </IconButton>
              <IconLink href={buildPolygonscanTokenUrl(selectedToken.address)} label={dictionary.funding.openToken} />
            </>
          }
        />
        <ReviewRow
          label={dictionary.funding.transferAmount}
          value={amountUnits ? `${amount} ${selectedToken.symbol}` : dictionary.common.pending}
        />
        <ReviewRow
          label={dictionary.funding.atomicAmount}
          value={amountUnits ? amountUnits.toString() : dictionary.common.pending}
        />
        <ReviewRow
          label={dictionary.funding.sourceWallet}
          value={sourceAddress ? formatShortAddress(sourceAddress) : dictionary.funding.notConfigured}
          action={
            sourceAddress ? (
              <IconButton
                label={dictionary.funding.copy}
                onClick={() => onCopy(sourceAddress, dictionary.funding.copiedSource)}
              >
                <Clipboard className="size-3.5" />
              </IconButton>
            ) : null
          }
        />
        <ReviewRow
          label={dictionary.funding.polymarketWallet}
          value={polymarketWalletAddress ? formatShortAddress(polymarketWalletAddress) : dictionary.funding.notConfigured}
          action={
            polymarketWalletAddress ? (
              <IconButton
                label={dictionary.funding.copy}
                onClick={() => onCopy(polymarketWalletAddress, dictionary.funding.copiedPolymarketWallet)}
              >
                <Clipboard className="size-3.5" />
              </IconButton>
            ) : null
          }
        />
        <ReviewRow
          label={dictionary.funding.bridgeAddress}
          value={bridgeDepositAddress ? formatShortAddress(bridgeDepositAddress) : dictionary.common.pending}
          action={
            bridgeDepositAddress ? (
              <IconButton
                label={dictionary.funding.copy}
                onClick={() => onCopy(bridgeDepositAddress, dictionary.funding.copiedBridgeAddress)}
              >
                <Clipboard className="size-3.5" />
              </IconButton>
            ) : null
          }
        />
        <ReviewRow
          label={dictionary.funding.status}
          value={submission.message ?? dictionary.funding.ready}
          action={
            submission.transfer?.tx_hash ? (
              <IconLink href={buildPolygonscanTxUrl(submission.transfer.tx_hash)} label={dictionary.funding.openTransaction} />
            ) : null
          }
        />
      </CardContent>
    </Card>
  );
}

function ReviewRow({
  label,
  value,
  action,
}: {
  label: string;
  value: string;
  action?: React.ReactNode;
}) {
  return (
    <div className="flex min-h-12 items-center justify-between gap-3 border-b border-border/60 py-2 last:border-b-0">
      <div className="min-w-0 space-y-1">
        <p className="text-xs text-muted-foreground">{label}</p>
        <p className="break-all font-mono text-xs text-foreground">{value}</p>
      </div>
      {action ? <div className="flex shrink-0 items-center gap-1">{action}</div> : null}
    </div>
  );
}

function IconButton({
  children,
  label,
  onClick,
}: {
  children: React.ReactNode;
  label: string;
  onClick: () => void;
}) {
  return (
    <Button aria-label={label} onClick={onClick} size="icon-sm" type="button" variant="ghost">
      {children}
    </Button>
  );
}

function IconLink({ href, label }: { href: string; label: string }) {
  return (
    <Button asChild aria-label={label} size="icon-sm" variant="ghost">
      <a href={href} rel="noreferrer" target="_blank">
        <ExternalLink className="size-3.5" />
      </a>
    </Button>
  );
}
