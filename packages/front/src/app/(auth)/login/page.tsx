import {
  getConsoleAuthMode,
  getRequiredConsoleRole,
  sanitizeNextPath,
} from "@/lib/console-auth";
import { MockSessionLoginPanel } from "@/components/auth/mock-session-login-panel";

type LoginPageProps = {
  searchParams: Promise<{
    next?: string | string[];
  }>;
};

export default async function LoginPage({ searchParams }: LoginPageProps) {
  const resolvedSearchParams = await searchParams;
  const nextPath = sanitizeNextPath(resolvedSearchParams.next);
  const requiredRole = getRequiredConsoleRole(nextPath) ?? "viewer";
  const authMode = getConsoleAuthMode(process.env.POLYEDGE_CONSOLE_AUTH);

  return <MockSessionLoginPanel authMode={authMode} nextPath={nextPath} requiredRole={requiredRole} />;
}
