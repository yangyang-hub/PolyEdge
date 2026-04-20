import "server-only";

import { cookies } from "next/headers";

import {
  CONSOLE_ROLE_COOKIE,
  CONSOLE_USER_COOKIE,
  type ConsoleRole,
  hasRequiredConsoleRole,
  normalizeConsoleRole,
} from "@/lib/console-auth";

export type ConsoleSession = {
  role: ConsoleRole | null;
  displayName: string | null;
};

export async function readConsoleSession(): Promise<ConsoleSession> {
  const cookieStore = await cookies();
  const role = normalizeConsoleRole(cookieStore.get(CONSOLE_ROLE_COOKIE)?.value);
  const displayName = cookieStore.get(CONSOLE_USER_COOKIE)?.value
    ? decodeURIComponent(cookieStore.get(CONSOLE_USER_COOKIE)!.value)
    : null;

  return {
    role,
    displayName,
  };
}

export async function assertConsoleRole(requiredRole: ConsoleRole): Promise<ConsoleSession> {
  const session = await readConsoleSession();

  if (!session.role || !hasRequiredConsoleRole(session.role, requiredRole)) {
    throw new Error(`Console session does not satisfy required role: ${requiredRole}`);
  }

  return session;
}
