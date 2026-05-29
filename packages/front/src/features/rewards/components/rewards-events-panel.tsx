"use client";

import { useMemo, useState } from "react";

import { Tabs, TabsContent, TabsList, TabsTrigger } from "@/components/ui/tabs";
import type { RewardFillDto, RewardRiskEventDto } from "@/lib/contracts/dto";
import { useI18n } from "@/lib/i18n/client";

import { eventCategory } from "../lib/rewards-helpers";
import type { EventCategory } from "../types";
import { EventsTable, FillsTable } from "./rewards-tables";

export function EventsPanel({
  events,
  fills,
}: {
  events: RewardRiskEventDto[];
  fills: RewardFillDto[];
}) {
  const { dictionary } = useI18n();
  const [category, setCategory] = useState<EventCategory>("all");

  const filteredEvents = useMemo(
    () =>
      category === "all"
        ? events
        : events.filter((event) => eventCategory(event.event_type) === category),
    [events, category],
  );

  return (
    <Tabs
      value={category}
      onValueChange={(value) => setCategory(value as EventCategory)}
      className="gap-4"
    >
      <TabsList>
        <TabsTrigger value="all">{dictionary.rewards.eventsAll}</TabsTrigger>
        <TabsTrigger value="placements">{dictionary.rewards.eventsPlacements}</TabsTrigger>
        <TabsTrigger value="cancels">{dictionary.rewards.eventsCancels}</TabsTrigger>
        <TabsTrigger value="fills">{dictionary.rewards.eventsFills}</TabsTrigger>
        <TabsTrigger value="rewards">{dictionary.rewards.eventsRewards}</TabsTrigger>
      </TabsList>
      <TabsContent value="all">
        <EventsTable events={filteredEvents} />
      </TabsContent>
      <TabsContent value="placements">
        <EventsTable events={filteredEvents} />
      </TabsContent>
      <TabsContent value="cancels">
        <EventsTable events={filteredEvents} />
      </TabsContent>
      <TabsContent value="fills">
        <FillsTable fills={fills} />
      </TabsContent>
      <TabsContent value="rewards">
        <EventsTable events={filteredEvents} />
      </TabsContent>
    </Tabs>
  );
}
