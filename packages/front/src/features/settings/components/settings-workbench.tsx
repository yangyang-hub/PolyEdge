import { MeterBar } from "@/components/shared/meter-bar";
import { PageHeader } from "@/components/shared/page-header";
import { StatusPill } from "@/components/shared/status-pill";
import { Card, CardAction, CardContent, CardDescription, CardHeader, CardTitle } from "@/components/ui/card";
import { Table, TableBody, TableCell, TableHead, TableHeader, TableRow } from "@/components/ui/table";
import { RuntimeConfigPanel } from "@/features/settings/components/runtime-config-panel";
import type { getSettingsPageData } from "@/features/settings/loaders/settings-page-data";
import { dictionary, formatMessage } from "@/lib/i18n/dictionaries";

type SettingsPageData = Awaited<ReturnType<typeof getSettingsPageData>>;

type SettingsWorkbenchProps = {
  data: SettingsPageData;
  format?: (template: string, values?: Record<string, string | number>) => string;
};

export function SettingsWorkbench({ data }: SettingsWorkbenchProps) {
  const format = formatMessage;
  const docs = [
    { title: dictionary.docs.systemDesign, path: "doc/polyedge-design.md" },
    { title: dictionary.docs.frontendDesign, path: "doc/polyedge-frontend-design.md" },
    { title: dictionary.docs.uiStack, path: "doc/polyedge-frontend-ui-stack.md" },
  ];

  return (
    <div className="space-y-6">
      <PageHeader
        eyebrow={dictionary.settings.eyebrow}
        title={dictionary.settings.title}
        description={dictionary.settings.description}
        actions={
          <StatusPill tone={data.backendMode === "live" ? "success" : "warning"}>
            {data.backendMode}
          </StatusPill>
        }
      />

      <section className="grid gap-4 md:grid-cols-2">
        <RuntimeConfigPanel entries={data.runtimeConfig} />

        <Card className="md:col-span-2">
          <CardHeader>
            <CardTitle>{dictionary.settings.dataSourceHealth}</CardTitle>
            <CardDescription>{dictionary.settings.sourceHealthDescription}</CardDescription>
            <CardAction>
              <StatusPill tone={data.sourceHealthSummary.tone}>{data.sourceHealthSummary.label}</StatusPill>
            </CardAction>
          </CardHeader>
          <CardContent>
            {data.sourceHealth.length > 0 ? (
              <Table>
                <TableHeader>
                  <TableRow>
                    <TableHead>{dictionary.settings.source}</TableHead>
                    <TableHead>{dictionary.settings.type}</TableHead>
                    <TableHead className="min-w-36">{dictionary.settings.health}</TableHead>
                    <TableHead>{dictionary.settings.fetched}</TableHead>
                    <TableHead>{dictionary.settings.inserted}</TableHead>
                    <TableHead>{dictionary.settings.deduped}</TableHead>
                    <TableHead>{dictionary.settings.failures}</TableHead>
                    <TableHead>{dictionary.settings.updated}</TableHead>
                    <TableHead>{dictionary.settings.error}</TableHead>
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
                            <span className="text-xs text-muted-foreground">
                              {source.reliabilityLabel} {dictionary.settings.rel}
                            </span>
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
                          <p className="text-muted-foreground">
                            {dictionary.settings.ok} {source.lastSuccessLabel}
                          </p>
                        </div>
                      </TableCell>
                      <TableCell className="max-w-56 whitespace-normal text-xs text-muted-foreground">
                        {source.lastError ? `${source.lastErrorLabel} - ${source.lastError}` : dictionary.common.none}
                      </TableCell>
                    </TableRow>
                  ))}
                </TableBody>
              </Table>
            ) : (
              <p className="text-sm text-muted-foreground">{dictionary.settings.noSourceHealth}</p>
            )}
          </CardContent>
        </Card>

        <Card className="md:col-span-2">
          <CardHeader>
            <CardTitle>{dictionary.settings.recentRawNews}</CardTitle>
            <CardDescription>{dictionary.settings.rawNewsDescription}</CardDescription>
          </CardHeader>
          <CardContent>
            {data.rawNews.length > 0 ? (
              <Table>
                <TableHeader>
                  <TableRow>
                    <TableHead>{dictionary.settings.source}</TableHead>
                    <TableHead>{dictionary.settings.titleColumn}</TableHead>
                    <TableHead>{dictionary.settings.eventTime}</TableHead>
                    <TableHead>{dictionary.settings.ingested}</TableHead>
                    <TableHead>{dictionary.settings.trace}</TableHead>
                  </TableRow>
                </TableHeader>
                <TableBody>
                  {data.rawNews.map((event) => (
                    <TableRow key={event.id}>
                      <TableCell>
                        <div className="space-y-1">
                          <p className="font-mono text-xs text-foreground">{event.source}</p>
                          <StatusPill tone="neutral">{event.typeLabel}</StatusPill>
                        </div>
                      </TableCell>
                      <TableCell className="max-w-xl whitespace-normal">
                        <div className="space-y-1">
                          {event.url ? (
                            <a
                              href={event.url}
                              className="text-sm font-medium text-foreground underline-offset-4 hover:underline"
                              target="_blank"
                              rel="noreferrer"
                            >
                              {event.title}
                            </a>
                          ) : (
                            <p className="text-sm font-medium text-foreground">{event.title}</p>
                          )}
                          <p className="text-xs text-muted-foreground">
                            {event.externalId ?? event.author ?? event.id}
                          </p>
                        </div>
                      </TableCell>
                      <TableCell>
                        <div className="space-y-1 text-xs">
                          <p className="text-foreground">{event.eventTimeLabel}</p>
                          <p className="text-muted-foreground">
                            {dictionary.common.published} {event.publishedAtLabel}
                          </p>
                        </div>
                      </TableCell>
                      <TableCell className="text-xs text-muted-foreground">{event.ingestedAtLabel}</TableCell>
                      <TableCell className="font-mono text-xs text-muted-foreground">{event.traceId}</TableCell>
                    </TableRow>
                  ))}
                </TableBody>
              </Table>
            ) : (
              <p className="text-sm text-muted-foreground">{dictionary.settings.noRawNews}</p>
            )}
          </CardContent>
        </Card>

        <Card>
          <CardHeader>
            <CardTitle className="font-heading text-base">{dictionary.settings.documentationLinks}</CardTitle>
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
            <CardTitle className="font-heading text-base">{dictionary.settings.buildTargets}</CardTitle>
          </CardHeader>
          <CardContent className="space-y-2 text-sm text-muted-foreground">
            <p>
              1. {format(dictionary.settings.buildTargetApi, {
                state: data.apiBaseUrl ? `-> ${data.apiBaseUrl}` : dictionary.settings.buildTargetApiUnset,
              })}
            </p>
            <p>2. {format(dictionary.settings.buildTargetBackendMode, { mode: data.backendMode })}</p>
            <p>3. {format(dictionary.settings.buildTargetAuthMode, { mode: data.consoleAuthMode })}</p>
            <p>4. {dictionary.settings.buildTargetLiveMode}</p>
            <p>5. {dictionary.settings.buildTargetStepUp}</p>
          </CardContent>
        </Card>
      </section>
    </div>
  );
}
