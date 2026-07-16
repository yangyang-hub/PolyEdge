import { shared } from "./dictionaries/shared";
import { enums } from "./dictionaries/enums";
import { v3 } from "./dictionaries/v3";

const _dictionary = {
  ...shared,
  ...enums,
  ...v3,
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
