"use client";

import { useMemo, useState } from "react";

import { Tabs, TabsContent, TabsList, TabsTrigger } from "@/components/ui/tabs";
import type { RewardFillDto, RewardRiskEventDto } from "@/lib/contracts/dto";
import { dictionary } from "@/lib/i18n/dictionaries";

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
  const [category, setCategory] = useState<EventCategory>("all");

  const filteredEvents = useMemo(() => {
    if (category === "all") {
      // "全部" tab excludes fill-classified events because those are
      // already shown as dedicated fill records in the "成交" tab.
      return events.filter((event) => eventCategory(event.event_type) !== "fills");
    }
    if (category === "fills") {
      // "成交" tab renders FillsTable instead, so this branch is unused.
      return events;
    }
    return events.filter((event) => eventCategory(event.event_type) === category);
  }, [events, category]);

  return (
    <Tabs
      value={category}
      onValueChange={(value) => setCategory(value as EventCategory)}
      className="gap-4"
    >
      <TabsList className="h-auto flex-wrap justify-start">
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
