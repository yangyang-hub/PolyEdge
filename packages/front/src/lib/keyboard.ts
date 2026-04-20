import type { KeyboardEvent } from "react";

export function isKeyboardSelect(event: KeyboardEvent<HTMLElement>): boolean {
  return event.key === "Enter" || event.key === " ";
}
