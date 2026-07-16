"use client";
import { useEffect, useState } from "react";
import { PageHeader } from "@/components/shared/page-header";
import { Card, CardContent } from "@/components/ui/card";
import { listAdminFinance } from "@/lib/api/admin";
import type { AdminFinanceDto } from "@/lib/contracts/dto";
export function FinanceWorkbench() { const [rows,setRows]=useState<AdminFinanceDto[]>([]); const [error,setError]=useState(""); useEffect(()=>{void listAdminFinance().then(r=>setRows(r.data)).catch(e=>setError(e instanceof Error?e.message:"财务数据加载失败"));},[]); return <div className="space-y-8"><PageHeader eyebrow="管理员" title="全局资金与收益" description="按用户聚合权益、可用资金与已实现/未实现收益；估值缺失时明确标记。"/>{error?<p className="text-destructive">{error}</p>:null}<div className="grid gap-4 lg:grid-cols-2">{rows.map(r=><Card key={r.user_id}><CardContent className="grid gap-3 p-5 sm:grid-cols-2"><div className="sm:col-span-2"><p className="font-medium">{r.display_name}</p><p className="text-xs text-muted-foreground">@{r.username} · {r.wallet_count} 个钱包</p></div><Metric label="总权益" value={r.equity}/><Metric label="可用资金" value={r.available_collateral}/><Metric label="已实现 PnL" value={r.realized_pnl}/><Metric label="未实现 PnL" value={r.unrealized_pnl}/><Metric label="总 PnL" value={r.total_pnl}/><Metric label="估值状态" value={r.valuation_complete?"完整":"不完整"}/></CardContent></Card>)}</div></div>; }
function Metric({label,value}:{label:string;value:string}){return <div><p className="text-xs text-muted-foreground">{label}</p><p className="font-mono text-lg font-semibold">{value}</p></div>}
