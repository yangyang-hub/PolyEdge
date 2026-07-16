"use client";

import { useEffect, useState, useTransition } from "react";
import { PageHeader } from "@/components/shared/page-header";
import { Button } from "@/components/ui/button";
import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card";
import { Input } from "@/components/ui/input";
import { createUser, listAdminUsers, reissueActivationToken, updateAdminUser } from "@/lib/api/admin";
import type { AdminUserDto, UserRole, UserStatus } from "@/lib/contracts/dto";

export function UsersWorkbench() {
  const [users, setUsers] = useState<AdminUserDto[]>([]);
  const [username, setUsername] = useState("");
  const [displayName, setDisplayName] = useState("");
  const [role, setRole] = useState<UserRole>("read_only");
  const [activationLink, setActivationLink] = useState("");
  const [error, setError] = useState("");
  const [pending, startTransition] = useTransition();

  const reload = () => void listAdminUsers()
    .then((response) => setUsers(response.data))
    .catch((cause) => setError(cause instanceof Error ? cause.message : "用户加载失败"));

  useEffect(reload, []);

  const showActivationLink = (token: string) => {
    setActivationLink(`${window.location.origin}/activate#token=${encodeURIComponent(token)}`);
  };

  const submit = () => startTransition(async () => {
    try {
      setError("");
      const response = await createUser({ username, display_name: displayName, role });
      showActivationLink(response.data.activation_token);
      setUsername("");
      setDisplayName("");
      reload();
    } catch (cause) {
      setError(cause instanceof Error ? cause.message : "创建失败");
    }
  });

  const reissue = (userId: number) => startTransition(async () => {
    try {
      setError("");
      const response = await reissueActivationToken(userId);
      showActivationLink(response.data.activation_token);
    } catch (cause) {
      setError(cause instanceof Error ? cause.message : "重新签发失败");
    }
  });

  const updateUser = (userId: number, body: { role?: UserRole; status?: UserStatus }) => startTransition(async () => {
    try {
      setError("");
      await updateAdminUser(userId, body);
      reload();
    } catch (cause) {
      setError(cause instanceof Error ? cause.message : "用户更新失败");
    }
  });

  return <div className="space-y-8">
    <PageHeader eyebrow="管理员" title="用户与权限" description="只有管理员可以创建用户；激活链接只显示一次。" />
    <Card>
      <CardHeader><CardTitle>创建用户</CardTitle></CardHeader>
      <CardContent className="grid gap-4 md:grid-cols-4">
        <Input placeholder="用户名" value={username} onChange={(event) => setUsername(event.target.value)} />
        <Input placeholder="显示名称" value={displayName} onChange={(event) => setDisplayName(event.target.value)} />
        <select className="h-9 rounded-md border bg-background px-3 text-sm" value={role} onChange={(event) => setRole(event.target.value as UserRole)}>
          <option value="read_only">只读</option><option value="market_editor">市场录入</option><option value="admin">管理员</option>
        </select>
        <Button disabled={pending || !username || !displayName} onClick={submit}>创建</Button>
        {activationLink ? <p className="md:col-span-4 break-all rounded-md bg-muted p-3 font-mono text-xs">一次性激活链接：{activationLink}</p> : null}
        {error ? <p className="md:col-span-4 text-sm text-destructive">{error}</p> : null}
      </CardContent>
    </Card>
    <Card>
      <CardHeader><CardTitle>全部用户</CardTitle></CardHeader>
      <CardContent className="space-y-2">{users.map((user) => <div key={user.id} className="grid gap-2 rounded-md border p-3 text-sm md:grid-cols-5">
        <span>{user.display_name}<br /><span className="text-muted-foreground">@{user.username}</span></span>
        <select className="h-9 rounded-md border bg-background px-2" value={user.role} disabled={pending || user.auth_source === "environment_admin"} onChange={(event) => updateUser(user.id, { role: event.target.value as UserRole })}>
          <option value="read_only">只读</option><option value="market_editor">市场录入</option><option value="admin">管理员</option>
        </select>
        <select className="h-9 rounded-md border bg-background px-2" value={user.status} disabled={pending || user.auth_source === "environment_admin"} onChange={(event) => updateUser(user.id, { status: event.target.value as UserStatus })}>
          <option value="pending">待激活</option><option value="active">启用</option><option value="disabled">禁用</option><option value="locked">锁定</option>
        </select>
        <span>{user.auth_source}</span>
        <span className="flex items-center justify-between gap-2">凭证版本 {user.credential_version}
          {user.status === "pending" && user.auth_source === "local" ? <Button size="sm" variant="outline" disabled={pending} onClick={() => reissue(user.id)}>重签激活链接</Button> : null}
        </span>
      </div>)}</CardContent>
    </Card>
  </div>;
}
