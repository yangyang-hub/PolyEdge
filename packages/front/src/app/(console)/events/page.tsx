"use client";

import { ClientDataBoundary } from "@/components/shared/client-data-boundary";
import { EventsWorkbench } from "@/features/events/components/events-workbench";
import { getEventsPageData } from "@/features/events/loaders/events-page-data";

export default function EventsPage() {
  return (
    <ClientDataBoundary load={getEventsPageData}>
      {(data) => <EventsWorkbench data={data} />}
    </ClientDataBoundary>
  );
}
