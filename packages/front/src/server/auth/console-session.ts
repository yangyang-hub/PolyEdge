import "server-only";

import {
  type ConsoleRole,
  hasRequiredConsoleRole,
} from "@/lib/console-auth";

export type ConsoleSession = {
  role: ConsoleRole | null;
  displayName: string | null;
};

export async function readConsoleSession(): Promise<ConsoleSession> {
  return {
    role: "admin",
    displayName: "Local Console",
  };
}

export async function assertConsoleRole(requiredRole: ConsoleRole): Promise<ConsoleSession> {
  const session = await readConsoleSession();

  if (!session.role || !hasRequiredConsoleRole(session.role, requiredRole)) {
    throw new Error(`Console session does not satisfy required role: ${requiredRole}`);
  }

  return session;
}
