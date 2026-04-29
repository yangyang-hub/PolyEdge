import { MeterBar } from "@/components/shared/meter-bar";
import { Card, CardAction, CardContent, CardDescription, CardHeader, CardTitle } from "@/components/ui/card";
import { Table, TableBody, TableCell, TableHead, TableHeader, TableRow } from "@/components/ui/table";
import { getSettingsPageData } from "@/features/settings/loaders/settings-page-data";
import { PageHeader } from "@/components/shared/page-header";
import { StatusPill } from "@/components/shared/status-pill";

const docs = [
  { title: "System Design", path: "doc/polyedge-design.md" },
  { title: "Frontend Design", path: "doc/polyedge-frontend-design.md" },
  { title: "Prototype Design", path: "doc/polyedge-prototype-design.md" },
  { title: "UI Stack", path: "doc/polyedge-frontend-ui-stack.md" },
];

export default async function SettingsPage() {
  const data = await getSettingsPageData();

  return (
    <div className="space-y-6">
      <PageHeader
        eyebrow="Configuration"
        title="Settings"
        description="Placeholder route for system configuration, permission management and implementation notes."
        actions={
          <StatusPill tone={data.backendMode === "live" ? "success" : "warning"}>
            {data.backendMode}
          </StatusPill>
        }
      />

      <section className="grid gap-4 md:grid-cols-2">
        <Card className="md:col-span-2">
          <CardHeader>
            <CardTitle>Data Source Health</CardTitle>
            <CardDescription>Latest ingestion counters and source degradation state.</CardDescription>
            <CardAction>
              <StatusPill tone={data.sourceHealthSummary.tone}>{data.sourceHealthSummary.label}</StatusPill>
            </CardAction>
          </CardHeader>
          <CardContent>
            {data.sourceHealth.length > 0 ? (
              <Table>
                <TableHeader>
                  <TableRow>
                    <TableHead>Source</TableHead>
                    <TableHead>Type</TableHead>
                    <TableHead className="min-w-36">Health</TableHead>
                    <TableHead>Fetched</TableHead>
                    <TableHead>Inserted</TableHead>
                    <TableHead>Deduped</TableHead>
                    <TableHead>Failures</TableHead>
                    <TableHead>Updated</TableHead>
                    <TableHead>Error</TableHead>
                  </TableRow>
                </TableHeader>
                <TableBody>
                  {data.sourceHealth.map((source) => (
                    <TableRow key={source.source}>
                      <TableCell>
                        <div className="space-y-1">
                          <p className="font-mono text-xs text-foreground">{source.source}</p>
                          <p className="text-xs text-muted-foreground">{source.enabledLabel}</p>
                        </div>
                      </TableCell>
                      <TableCell>
                        <StatusPill tone="neutral">{source.typeLabel}</StatusPill>
                      </TableCell>
                      <TableCell>
                        <div className="space-y-2">
                          <div className="flex items-center justify-between gap-3">
                            <StatusPill tone={source.tone}>{source.healthScoreLabel}</StatusPill>
                            <span className="text-xs text-muted-foreground">{source.reliabilityLabel} rel</span>
                          </div>
                          <MeterBar value={source.healthScoreWidth} tone={source.tone} />
                        </div>
                      </TableCell>
                      <TableCell>{source.fetchedLabel}</TableCell>
                      <TableCell>{source.insertedLabel}</TableCell>
                      <TableCell>{source.dedupedLabel}</TableCell>
                      <TableCell>{source.consecutiveFailures}</TableCell>
                      <TableCell>
                        <div className="space-y-1 text-xs">
                          <p className="text-foreground">{source.updatedAtLabel}</p>
                          <p className="text-muted-foreground">ok {source.lastSuccessLabel}</p>
                        </div>
                      </TableCell>
                      <TableCell className="max-w-56 whitespace-normal text-xs text-muted-foreground">
                        {source.lastError ? `${source.lastErrorLabel} - ${source.lastError}` : "none"}
                      </TableCell>
                    </TableRow>
                  ))}
                </TableBody>
              </Table>
            ) : (
              <p className="text-sm text-muted-foreground">No source health records have been ingested yet.</p>
            )}
          </CardContent>
        </Card>

        <Card>
          <CardHeader>
            <CardTitle className="font-heading text-base">Documentation Links</CardTitle>
          </CardHeader>
          <CardContent className="space-y-3">
            {docs.map((doc) => (
              <div key={doc.title} className="rounded-sm border border-border/70 bg-card p-3">
                <p className="text-sm font-medium text-foreground">{doc.title}</p>
                <p className="mt-1 font-mono text-xs text-muted-foreground">{doc.path}</p>
              </div>
            ))}
          </CardContent>
        </Card>

        <Card>
          <CardHeader>
            <CardTitle className="font-heading text-base">Next Build Targets</CardTitle>
          </CardHeader>
          <CardContent className="space-y-2 text-sm text-muted-foreground">
            <p>1. `POLYEDGE_API_BASE_URL` {data.apiBaseUrl ? `-> ${data.apiBaseUrl}` : "is unset, using typed mock envelopes"}.</p>
            <p>2. Backend mode resolves to `{data.backendMode}` from that value.</p>
            <p>3. `POLYEDGE_CONSOLE_AUTH` currently resolves to `{data.consoleAuthMode}`.</p>
            <p>4. Local live mode can use `POLYEDGE_INTERNAL_AUTH_DEV_BYPASS=1`; signed mode requires `POLYEDGE_INTERNAL_AUTH_KID` and `POLYEDGE_INTERNAL_AUTH_PRIVATE_KEY`.</p>
            <p>5. Protected actions verify `POLYEDGE_CONSOLE_STEP_UP_CODE` before sending step-up scopes downstream.</p>
          </CardContent>
        </Card>
      </section>
    </div>
  );
}
