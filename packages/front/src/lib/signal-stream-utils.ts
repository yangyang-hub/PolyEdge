import type { RiskStreamPayload, SignalStreamPayload } from "@/lib/contracts/realtime";

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

export function patchApprovalField<T extends { id: string }>(
  items: T[],
  payload: RiskStreamPayload,
  field: keyof T,
): T[] {
  if (
    payload.approval_type !== "signal" ||
    !payload.approval_resource_id ||
    !payload.approval_status
  ) {
    return items;
  }

  return items.map((item) =>
    item.id === payload.approval_resource_id
      ? { ...item, [field]: payload.approval_status === "pending" }
      : item,
  );
}
