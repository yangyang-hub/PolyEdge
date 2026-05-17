"use client";

import { createContext, useContext } from "react";

import {
  type Dictionary,
  formatMessage,
  translateEnum,
} from "@/lib/i18n/dictionaries";
import type { Locale } from "@/lib/i18n/locales";

type I18nContextValue = {
  locale: Locale;
  dictionary: Dictionary;
  format: (template: string, values?: Record<string, string | number>) => string;
  enumLabel: (value: string) => string;
};

const I18nContext = createContext<I18nContextValue | null>(null);

export function I18nProvider({
  children,
  locale,
  dictionary,
}: {
  children: React.ReactNode;
  locale: Locale;
  dictionary: Dictionary;
}) {
  return (
    <I18nContext.Provider
      value={{
        locale,
        dictionary,
        format: formatMessage,
        enumLabel: (value) => translateEnum(dictionary, value),
      }}
    >
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
