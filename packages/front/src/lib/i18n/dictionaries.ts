import type { Locale } from "@/lib/i18n/locales";
import { sharedEn, sharedZh } from "./dictionaries/shared";
import { enumsEn, enumsZh } from "./dictionaries/enums";
import { dashboardEn, dashboardZh } from "./dictionaries/dashboard";
import { marketsEn, marketsZh } from "./dictionaries/markets";
import { signalsEn, signalsZh } from "./dictionaries/signals";
import { positionsEn, positionsZh } from "./dictionaries/positions";
import { riskEn, riskZh } from "./dictionaries/risk";
import { radarEn, radarZh } from "./dictionaries/radar";
import { rewardsEn, rewardsZh } from "./dictionaries/rewards";
import { opsEn, opsZh } from "./dictionaries/ops";

const enUS = {
  localeName: "English",
  shortLocaleName: "EN",
  ...sharedEn,
  ...enumsEn,
  ...dashboardEn,
  ...marketsEn,
  ...signalsEn,
  ...positionsEn,
  ...riskEn,
  ...radarEn,
  ...rewardsEn,
  ...opsEn,
} as const;

type DeepStringRecord<T> = {
  readonly [K in keyof T]: T[K] extends string
    ? string
    : T[K] extends object
      ? DeepStringRecord<T[K]>
      : T[K];
};

export type Dictionary = DeepStringRecord<typeof enUS>;

const zhCN = {
  ...enUS,
  localeName: "中文",
  shortLocaleName: "中",
  ...sharedZh,
  ...enumsZh,
  ...dashboardZh,
  ...marketsZh,
  ...signalsZh,
  ...positionsZh,
  ...riskZh,
  ...radarZh,
  ...rewardsZh,
  ...opsZh,
} satisfies Dictionary;

export const dictionaries = {
  "en-US": enUS,
  "zh-CN": zhCN,
} satisfies Record<Locale, Dictionary>;

export function getDictionary(locale: Locale): Dictionary {
  return dictionaries[locale];
}

export function formatMessage(template: string, values?: Record<string, string | number>): string {
  if (!values) {
    return template;
  }

  return Object.entries(values).reduce(
    (message, [key, value]) => message.replaceAll(`{${key}}`, String(value)),
    template,
  );
}

export function translateEnum(dictionary: Dictionary, value: string): string {
  return dictionary.enums[value as keyof Dictionary["enums"]] ?? value.replaceAll("_", " ");
}
