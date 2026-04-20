import type { ConsoleRole } from "@/lib/console-auth";
import { getRequiredConsoleRole, hasRequiredConsoleRole } from "@/lib/console-auth";

export type ConsoleRouteAccess = {
  requiredRole: ConsoleRole | null;
  allowed: boolean;
};

export function resolveConsoleRouteAccess(
  pathname: string,
  currentRole: ConsoleRole | null,
): ConsoleRouteAccess {
  const requiredRole = getRequiredConsoleRole(pathname);

  if (!requiredRole) {
    return {
      requiredRole: null,
      allowed: true,
    };
  }

  return {
    requiredRole,
    allowed: currentRole ? hasRequiredConsoleRole(currentRole, requiredRole) : false,
  };
}
