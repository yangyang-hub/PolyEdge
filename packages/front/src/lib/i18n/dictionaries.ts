import { shared } from "./dictionaries/shared";
import { enums } from "./dictionaries/enums";
import { dashboard } from "./dictionaries/dashboard";
import { markets } from "./dictionaries/markets";
import { rewards } from "./dictionaries/rewards";
import { ops } from "./dictionaries/ops";
import { funding } from "./dictionaries/funding";

const _dictionary = {
  ...shared,
  ...enums,
  ...dashboard,
  ...markets,
  ...rewards,
  ...ops,
  ...funding,
} as const;

type DeepStringRecord<T> = {
  readonly [K in keyof T]: T[K] extends string
    ? string
    : T[K] extends object
      ? DeepStringRecord<T[K]>
      : T[K];
};

export type Dictionary = DeepStringRecord<typeof _dictionary>;

export const dictionary: Dictionary = _dictionary;

export function formatMessage(template: string, values?: Record<string, string | number>): string {
  if (!values) {
    return template;
  }

  return Object.entries(values).reduce(
    (message, [key, value]) => message.replaceAll(`{${key}}`, String(value)),
    template,
  );
}

export function translateEnum(value: string): string {
  return dictionary.enums[value as keyof Dictionary["enums"]] ?? value.replaceAll("_", " ");
}
