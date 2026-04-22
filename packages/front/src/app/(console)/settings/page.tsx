import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card";
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
            <p>4. Live backend requests also require `POLYEDGE_INTERNAL_AUTH_KID` and `POLYEDGE_INTERNAL_AUTH_PRIVATE_KEY` for Next.js server-side token signing.</p>
          </CardContent>
        </Card>
      </section>
    </div>
  );
}
