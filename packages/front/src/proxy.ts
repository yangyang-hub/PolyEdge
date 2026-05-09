import { NextResponse } from "next/server";
import type { NextRequest } from "next/server";

import {
  CONSOLE_ROLE_COOKIE,
  getConsoleAuthMode,
  normalizeConsoleRole,
} from "@/lib/console-auth";
import { resolveConsoleRouteAccess } from "@/server/permissions/console-access";

function redirectTo(request: NextRequest, pathname: string, params: Record<string, string>) {
  const url = request.nextUrl.clone();
  url.pathname = pathname;
  url.search = "";

  for (const [key, value] of Object.entries(params)) {
    url.searchParams.set(key, value);
  }

  return NextResponse.redirect(url);
}

export function proxy(request: NextRequest) {
  const authMode = getConsoleAuthMode(process.env.POLYEDGE_CONSOLE_AUTH);
  const currentRole = normalizeConsoleRole(request.cookies.get(CONSOLE_ROLE_COOKIE)?.value);
  const { allowed, requiredRole } = resolveConsoleRouteAccess(request.nextUrl.pathname, currentRole);

  if (!requiredRole || authMode === "off") {
    return NextResponse.next();
  }

  const nextPath = `${request.nextUrl.pathname}${request.nextUrl.search}`;

  if (!currentRole) {
    return redirectTo(request, "/login", { next: nextPath });
  }

  if (!allowed) {
    return redirectTo(request, "/unauthorized", {
      next: nextPath,
      required: requiredRole,
      current: currentRole,
    });
  }

  return NextResponse.next();
}

export const config = {
  matcher: [
    "/dashboard/:path*",
    "/markets/:path*",
    "/events/:path*",
    "/signals/:path*",
    "/positions/:path*",
    "/risk/:path*",
    "/approvals/:path*",
    "/replay/:path*",
    "/settings/:path*",
  ],
};
