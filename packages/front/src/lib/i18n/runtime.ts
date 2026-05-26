import {
  type Dictionary,
  formatMessage,
  getDictionary,
  translateEnum,
} from "@/lib/i18n/dictionaries";
import { DEFAULT_LOCALE, type Locale } from "@/lib/i18n/locales";

export type I18nRuntime = {
  locale: Locale;
  dictionary: Dictionary;
  format: (template: string, values?: Record<string, string | number>) => string;
  enumLabel: (value: string) => string;
};

export function createI18nRuntime(locale: Locale = DEFAULT_LOCALE): I18nRuntime {
  const dictionary = getDictionary(locale);

  return {
    locale,
    dictionary,
    format: formatMessage,
    enumLabel: (value) => translateEnum(dictionary, value),
  };
}
