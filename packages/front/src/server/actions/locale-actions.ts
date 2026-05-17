"use server";

import { cookies } from "next/headers";

import { LOCALE_COOKIE, normalizeLocale, type Locale } from "@/lib/i18n/locales";

export async function setLocaleAction(locale: Locale): Promise<void> {
  const cookieStore = await cookies();

  cookieStore.set(LOCALE_COOKIE, normalizeLocale(locale), {
    path: "/",
    maxAge: 60 * 60 * 24 * 365,
    sameSite: "lax",
  });
}
