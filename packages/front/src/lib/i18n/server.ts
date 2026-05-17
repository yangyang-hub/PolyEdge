import "server-only";

import { cookies } from "next/headers";

import {
  type Dictionary,
  formatMessage,
  getDictionary,
  translateEnum,
} from "@/lib/i18n/dictionaries";
import { LOCALE_COOKIE, normalizeLocale, type Locale } from "@/lib/i18n/locales";

export async function getCurrentLocale(): Promise<Locale> {
  const cookieStore = await cookies();
  return normalizeLocale(cookieStore.get(LOCALE_COOKIE)?.value);
}

export async function getServerI18n(): Promise<{
  locale: Locale;
  dictionary: Dictionary;
  format: (template: string, values?: Record<string, string | number>) => string;
  enumLabel: (value: string) => string;
}> {
  const locale = await getCurrentLocale();
  const dictionary = getDictionary(locale);

  return {
    locale,
    dictionary,
    format: formatMessage,
    enumLabel: (value) => translateEnum(dictionary, value),
  };
}
