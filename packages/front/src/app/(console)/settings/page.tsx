"use client";

import { ClientDataBoundary } from "@/components/shared/client-data-boundary";
import { SettingsWorkbench } from "@/features/settings/components/settings-workbench";
import { getSettingsPageData } from "@/features/settings/loaders/settings-page-data";

export default function SettingsPage() {
  return (
    <ClientDataBoundary load={getSettingsPageData}>
      {(data) => (
        <SettingsWorkbench data={data} />
      )}
    </ClientDataBoundary>
  );
}
