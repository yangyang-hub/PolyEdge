export type ConsoleRole = "viewer" | "operator" | "risk_admin" | "admin";
export type ConsoleAuthMode = "off";

export const CONSOLE_ROLES: ConsoleRole[] = ["viewer", "operator", "risk_admin", "admin"];

export const CONSOLE_ROLE_LABELS: Record<ConsoleRole, string> = {
  viewer: "Viewer",
  operator: "Operator",
  risk_admin: "Risk Admin",
  admin: "Admin",
};

const CONSOLE_ROLE_RANK: Record<ConsoleRole, number> = {
  viewer: 0,
  operator: 1,
  risk_admin: 2,
  admin: 3,
};

const CONSOLE_ROUTE_REQUIREMENTS: Array<{ prefix: string; minRole: ConsoleRole }> = [
  { prefix: "/dashboard", minRole: "viewer" },
  { prefix: "/strategies", minRole: "operator" },
  { prefix: "/wallets", minRole: "operator" },
  { prefix: "/operations", minRole: "operator" },
  { prefix: "/settings", minRole: "admin" },
];

export function getConsoleAuthMode(rawValue: string | null | undefined): ConsoleAuthMode {
  void rawValue;
  return "off";
}

export function normalizeConsoleRole(rawValue: string | null | undefined): ConsoleRole | null {
  if (!rawValue) {
    return null;
  }

  return CONSOLE_ROLES.find((role) => role === rawValue) ?? null;
}

export function hasRequiredConsoleRole(currentRole: ConsoleRole, requiredRole: ConsoleRole): boolean {
  return CONSOLE_ROLE_RANK[currentRole] >= CONSOLE_ROLE_RANK[requiredRole];
}

function matchesRoute(pathname: string, prefix: string): boolean {
  return pathname === prefix || pathname.startsWith(`${prefix}/`);
}

export function getRequiredConsoleRole(pathname: string): ConsoleRole | null {
  const matchedRoute = CONSOLE_ROUTE_REQUIREMENTS.find(({ prefix }) => matchesRoute(pathname, prefix));
  return matchedRoute?.minRole ?? null;
}

export function sanitizeNextPath(rawValue: string | string[] | null | undefined): string {
  const value = Array.isArray(rawValue) ? rawValue[0] : rawValue;

  if (!value || !value.startsWith("/") || value.startsWith("//")) {
    return "/dashboard";
  }

  return value;
}
