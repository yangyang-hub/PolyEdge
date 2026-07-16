"use client";

import { useState, useTransition } from "react";

import { Button } from "@/components/ui/button";
import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card";
import { Input } from "@/components/ui/input";
import { login } from "@/lib/api/auth";
import { sanitizeNextPath } from "@/lib/console-auth";

export default function LoginPage() {
  const [username, setUsername] = useState("");
  const [password, setPassword] = useState("");
  const [error, setError] = useState("");
  const [pending, startTransition] = useTransition();

  const submit = () => startTransition(async () => {
    try {
      await login({ username, password });
      window.location.replace(sanitizeNextPath(new URLSearchParams(window.location.search).get("next")));
    } catch (reason) {
      setError(reason instanceof Error ? reason.message : "登录失败");
    }
  });

  return <Card className="w-full max-w-md">
    <CardHeader><CardTitle>登录 PolyEdge</CardTitle></CardHeader>
    <CardContent className="space-y-4">
      <label className="space-y-2 text-sm"><span>用户名</span><Input autoComplete="username" value={username} onChange={(event) => setUsername(event.target.value)} /></label>
      <label className="space-y-2 text-sm"><span>密码</span><Input type="password" autoComplete="current-password" value={password} onChange={(event) => setPassword(event.target.value)} onKeyDown={(event) => event.key === "Enter" && submit()} /></label>
      {error ? <p className="text-sm text-destructive">{error}</p> : null}
      <Button className="w-full" disabled={pending || !username.trim() || !password} onClick={submit}>{pending ? "登录中…" : "登录"}</Button>
    </CardContent>
  </Card>;
}
