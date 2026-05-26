"use client";

import { createContext, useCallback, useContext, useEffect, useMemo, useState, type ReactNode } from "react";

import type { Dictionary } from "@/lib/i18n/dictionaries";
import {
  LOCALE_COOKIE,
  normalizeLocale,
  type Locale,
} from "@/lib/i18n/locales";
import { createI18nRuntime } from "@/lib/i18n/runtime";

type I18nContextValue = {
  locale: Locale;
  dictionary: Dictionary;
  format: (template: string, values?: Record<string, string | number>) => string;
  enumLabel: (value: string) => string;
  setLocale: (locale: Locale) => void;
};

const I18nContext = createContext<I18nContextValue | null>(null);

function readLocaleCookie(): Locale | null {
  if (typeof document === "undefined") {
    return null;
  }

  const rawValue = document.cookie
    .split(";")
    .map((item) => item.trim())
    .find((item) => item.startsWith(`${LOCALE_COOKIE}=`))
    ?.slice(LOCALE_COOKIE.length + 1);

  return rawValue ? normalizeLocale(decodeURIComponent(rawValue)) : null;
}

function writeLocaleCookie(locale: Locale): void {
  document.cookie = `${LOCALE_COOKIE}=${encodeURIComponent(locale)}; Path=/; Max-Age=31536000; SameSite=Lax`;
}

export function I18nProvider({
  children,
  locale,
}: {
  children: ReactNode;
  locale: Locale;
  dictionary: Dictionary;
}) {
  const [activeLocale, setActiveLocale] = useState(locale);
  const runtime = useMemo(() => createI18nRuntime(activeLocale), [activeLocale]);
  const updateLocale = useCallback((nextLocale: Locale) => {
    writeLocaleCookie(nextLocale);
    setActiveLocale(nextLocale);
  }, []);

  useEffect(() => {
    const timeoutId = window.setTimeout(() => {
      const cookieLocale = readLocaleCookie();

      if (cookieLocale && cookieLocale !== activeLocale) {
        setActiveLocale(cookieLocale);
      }
    }, 0);

    return () => window.clearTimeout(timeoutId);
  }, [activeLocale]);

  useEffect(() => {
    document.documentElement.lang = activeLocale;
  }, [activeLocale]);

  const contextValue = useMemo<I18nContextValue>(
    () => ({
      ...runtime,
      setLocale: updateLocale,
    }),
    [runtime, updateLocale],
  );

  return (
    <I18nContext.Provider value={contextValue}>
      {children}
    </I18nContext.Provider>
  );
}

export function useI18n(): I18nContextValue {
  const context = useContext(I18nContext);

  if (!context) {
    throw new Error("useI18n must be used within I18nProvider.");
  }

  return context;
}
