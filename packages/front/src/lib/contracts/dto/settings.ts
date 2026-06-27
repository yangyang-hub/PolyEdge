import type { RuntimeConfigValueType } from "./primitives";

export type RuntimeConfigEntryDto = {
  key: string;
  section: string;
  field: string;
  label: string;
  env_name: string;
  value: string;
  default_value: string;
  value_type: RuntimeConfigValueType;
  options: string[];
  restart_required: boolean;
};

export type RuntimeConfigUpdateDto = {
  values: Record<string, string>;
};
