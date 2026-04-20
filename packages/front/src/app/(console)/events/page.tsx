import { EventsWorkbench } from "@/features/events/components/events-workbench";
import { getEventsPageData } from "@/features/events/loaders/events-page-data";

export default async function EventsPage() {
  const data = await getEventsPageData();

  return <EventsWorkbench data={data} />;
}
