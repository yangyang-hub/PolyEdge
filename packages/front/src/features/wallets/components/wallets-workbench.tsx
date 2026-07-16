"use client";

import { useEffect, useState, useTransition } from "react";

import { OperationFeedbackBanner } from "@/components/shared/operation-feedback-banner";
import { PageHeader } from "@/components/shared/page-header";
import { StatusPill } from "@/components/shared/status-pill";
import { Button } from "@/components/ui/button";
import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card";
import { Input } from "@/components/ui/input";
import { saveWallet, type OperationActionResult } from "@/lib/api/actions";
import { listWallets } from "@/lib/api/wallets";
import { encryptWalletSecret } from "@/lib/api/wallet-security";
import type {
  CreateWalletAccountRequest,
  WalletAccountData,
} from "@/lib/contracts/dto";
import { dictionary, translateEnum } from "@/lib/i18n/dictionaries";
import { canWriteMarkets, useAuth } from "@/components/shared/auth-provider";

type WalletForm = Omit<CreateWalletAccountRequest, "encrypted_secret">;
const INITIAL_FORM: WalletForm = {
  name: "",
  signer_address: "",
  funder_address: "",
  signature_type: 0,
  trading_enabled: false,
  risk_policy: {
    max_open_orders: 10,
    max_open_buy_notional: "100",
    max_total_position_notional: "200",
    max_market_position_notional: "50",
    max_order_notional: "20",
  },
};

export function WalletsWorkbench() {
  const d = dictionary.wallets;
  const { user } = useAuth();
  const writable = canWriteMarkets(user?.role);
  const [form, setForm] = useState<WalletForm>(INITIAL_FORM);
  const [privateKey, setPrivateKey] = useState("");
  const [wallets, setWallets] = useState<WalletAccountData[]>([]);
  const [loadError, setLoadError] = useState("");
  const [feedback, setFeedback] = useState<OperationActionResult | null>(null);
  const [isPending, startTransition] = useTransition();

  const reload = () => {
    void listWallets()
      .then((response) => {
        setWallets(response.data);
        setLoadError("");
      })
      .catch(() => setLoadError(d.loadFailed));
  };

  useEffect(reload, [d.loadFailed]);

  const setField = <K extends keyof WalletForm>(
    key: K,
    value: CreateWalletAccountRequest[K],
  ) => setForm((current) => ({ ...current, [key]: value }));

  const setRisk = (key: keyof WalletForm["risk_policy"], value: string) => {
    setForm((current) => ({
      ...current,
      risk_policy: {
        ...current.risk_policy,
        [key]: key === "max_open_orders" ? Number(value) : value,
      },
    }));
  };

  const submit = () => {
    startTransition(async () => {
      let encryptedSecret;
      try {
        if (!user) throw new Error("当前用户会话不可用");
        encryptedSecret = await encryptWalletSecret(user.id, { private_key: privateKey });
      }
      catch (error) { setFeedback({ ok: false, message: error instanceof Error ? error.message : "加密钱包私钥失败" }); return; }
      const result = await saveWallet({
        request: {
          ...form,
          encrypted_secret: encryptedSecret,
          operator_note: form.operator_note?.trim() || undefined,
        },
      });
      setFeedback(result);
      if (result.ok) {
        setForm(INITIAL_FORM);
        setPrivateKey("");
        reload();
      }
    });
  };

  return (
    <div className="space-y-8">
      <PageHeader eyebrow={d.eyebrow} title={d.title} description={d.description} />
      {feedback ? <OperationFeedbackBanner feedback={feedback} /> : null}
      <div className="grid gap-6 xl:grid-cols-[minmax(0,1fr)_minmax(0,1.1fr)]">
        {writable ? <Card>
          <CardHeader>
            <CardTitle>{d.add}</CardTitle>
          </CardHeader>
          <CardContent className="grid gap-4 md:grid-cols-2">
            <Field label={d.name} value={form.name} onChange={(value) => setField("name", value)} />
            <Field
              label={d.signerAddress}
              value={form.signer_address}
              onChange={(value) => setField("signer_address", value)}
            />
            <Field
              label={d.funder}
              value={form.funder_address}
              onChange={(value) => setField("funder_address", value)}
            />
            <label className="space-y-2 text-sm">
              <span>{d.signatureType}</span>
              <select
                className="h-9 w-full rounded-md border border-input bg-background px-3 text-sm"
                value={form.signature_type}
                onChange={(event) => setField("signature_type", Number(event.target.value))}
              >
                <option value={0}>0</option>
                <option value={1}>1</option>
                <option value={2}>2</option>
              </select>
            </label>
            <Field
              label="钱包私钥"
              type="password"
              value={privateKey}
              onChange={setPrivateKey}
              placeholder="仅在浏览器内加密后上传"
            />
            <label className="flex items-center gap-3 self-end rounded-md border p-3 text-sm">
              <input
                type="checkbox"
                checked={form.trading_enabled}
                onChange={(event) => setField("trading_enabled", event.target.checked)}
              />
              <span>{d.trading}</span>
            </label>
            <div className="grid gap-3 md:col-span-2 md:grid-cols-5">
              <Field label={d.maxOpenOrders} type="number" value={String(form.risk_policy.max_open_orders)} onChange={(value) => setRisk("max_open_orders", value)} />
              <Field label={d.maxOpenBuy} value={form.risk_policy.max_open_buy_notional} onChange={(value) => setRisk("max_open_buy_notional", value)} />
              <Field label={d.maxTotalPosition} value={form.risk_policy.max_total_position_notional} onChange={(value) => setRisk("max_total_position_notional", value)} />
              <Field label={d.maxMarketPosition} value={form.risk_policy.max_market_position_notional} onChange={(value) => setRisk("max_market_position_notional", value)} />
              <Field label={d.maxOrder} value={form.risk_policy.max_order_notional} onChange={(value) => setRisk("max_order_notional", value)} />
            </div>
            <Field
              label={d.operatorNote}
              value={form.operator_note ?? ""}
              onChange={(value) => setField("operator_note", value)}
              className="md:col-span-2"
            />
            <Button
              className="md:col-span-2"
              disabled={isPending || !privateKey.trim()}
              onClick={submit}
            >
              {isPending ? dictionary.common.submitting : d.save}
            </Button>
          </CardContent>
        </Card> : null}

        <Card>
          <CardHeader>
            <CardTitle>{d.accountState}</CardTitle>
          </CardHeader>
          <CardContent className="space-y-3">
            {loadError ? <p className="text-sm text-destructive">{loadError}</p> : null}
            {wallets.length === 0 && !loadError ? (
              <div className="rounded-lg border border-dashed p-8 text-center text-sm text-muted-foreground">
                {d.empty}
              </div>
            ) : null}
            {wallets.map(({ account, secret, risk_policy, state }) => (
              <div key={account.id} className="space-y-3 rounded-lg border p-4">
                <div className="flex flex-wrap items-start justify-between gap-3">
                  <div>
                    <p className="font-medium">{account.name}</p>
                    <p className="font-mono text-xs text-muted-foreground">{account.signer_address}</p>
                  </div>
                  <div className="flex gap-2">
                    <StatusPill tone={account.trading_enabled ? "success" : "neutral"}>
                      {account.trading_enabled ? d.active : d.disabled}
                    </StatusPill>
                    <StatusPill>{translateEnum(account.status)}</StatusPill>
                  </div>
                </div>
                <div className="grid gap-2 text-xs text-muted-foreground sm:grid-cols-2">
                  <p>{d.walletId}: {account.id}</p>
                  <p>{d.credential}: {secret.key_id} · v{secret.secret_version}</p>
                  <p>{d.availableCollateral}: {state.available_collateral}</p>
                  <p>{d.openBuyNotional}: {state.open_buy_notional}</p>
                  <p>{d.maxOpenOrders}: {risk_policy.max_open_orders}</p>
                  <p>{d.maxOrder}: {risk_policy.max_order_notional}</p>
                </div>
                {state.last_error ? <p className="text-xs text-destructive">{state.last_error}</p> : null}
              </div>
            ))}
          </CardContent>
        </Card>
      </div>
    </div>
  );
}

function Field({
  label,
  value,
  onChange,
  type = "text",
  placeholder,
  className,
}: {
  label: string;
  value: string;
  onChange: (value: string) => void;
  type?: string;
  placeholder?: string;
  className?: string;
}) {
  return (
    <label className={`space-y-2 text-sm ${className ?? ""}`}>
      <span>{label}</span>
      <Input
        type={type}
        value={value}
        placeholder={placeholder}
        onChange={(event) => onChange(event.target.value)}
      />
    </label>
  );
}
