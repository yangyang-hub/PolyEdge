import { redirect } from "next/navigation";

import { sanitizeNextPath } from "@/lib/console-auth";

type LoginPageProps = {
  searchParams: Promise<{
    next?: string | string[];
  }>;
};

export default async function LoginPage({ searchParams }: LoginPageProps) {
  const resolvedSearchParams = await searchParams;
  const nextPath = sanitizeNextPath(resolvedSearchParams.next);

  redirect(nextPath);
}
