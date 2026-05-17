export const LOCALES = ["zh-CN", "en-US"] as const;

export type Locale = (typeof LOCALES)[number];

export const DEFAULT_LOCALE: Locale = "zh-CN";
export const LOCALE_COOKIE = "polyedge_locale";

export function isLocale(value: unknown): value is Locale {
  return typeof value === "string" && LOCALES.some((locale) => locale === value);
}

export function normalizeLocale(value: unknown): Locale {
  return isLocale(value) ? value : DEFAULT_LOCALE;
}
