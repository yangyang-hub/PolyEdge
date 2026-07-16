"use client";

import { useState, useTransition } from "react";
import { Button } from "@/components/ui/button";
import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card";
import { Input } from "@/components/ui/input";
import { activate } from "@/lib/api/auth";

export default function ActivatePage() {
  const [password, setPassword] = useState("");
  const [confirmation, setConfirmation] = useState("");
  const [error, setError] = useState("");
  const [pending, startTransition] = useTransition();
  const submit = () => startTransition(async () => {
    const token = new URLSearchParams(window.location.hash.slice(1)).get("token") ?? "";
    if (!token) return setError("激活链接缺少令牌");
    if (password !== confirmation) return setError("两次输入的密码不一致");
    try {
      await activate({ token, password });
      window.location.replace("/login");
    } catch (reason) { setError(reason instanceof Error ? reason.message : "激活失败"); }
  });
  return <Card className="w-full max-w-md"><CardHeader><CardTitle>激活账户</CardTitle></CardHeader><CardContent className="space-y-4">
    <label className="space-y-2 text-sm"><span>设置密码</span><Input type="password" autoComplete="new-password" value={password} onChange={(e) => setPassword(e.target.value)} /></label>
    <label className="space-y-2 text-sm"><span>确认密码</span><Input type="password" autoComplete="new-password" value={confirmation} onChange={(e) => setConfirmation(e.target.value)} /></label>
    {error ? <p className="text-sm text-destructive">{error}</p> : null}
    <Button className="w-full" disabled={pending || password.length < 12} onClick={submit}>{pending ? "激活中…" : "激活并登录"}</Button>
  </CardContent></Card>;
}
