import type { SignalStreamPayload } from "@/lib/contracts/realtime";

export function upsertStreamedItem<T extends { id: string }>(
  items: T[],
  payload: SignalStreamPayload,
  build: (payload: SignalStreamPayload, current?: T) => T,
  eventType: string,
): T[] {
  const current = items.find((item) => item.id === payload.signal_id);
  const next = build(payload, current);

  if (current) {
    return items.map((item) => (item.id === payload.signal_id ? next : item));
  }

  if (eventType === "signal.created") {
    return [next, ...items];
  }

  return [...items, next];
}
