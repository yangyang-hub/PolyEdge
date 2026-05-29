import type {
  AlertSeverity,
  AlertStatus,
  BucketStatus,
  ResourceVersion,
  RuntimeConfigValueType,
  RuntimeEnvironment,
  RuntimeMode,
} from "./primitives";

export type RiskStateDto = ResourceVersion & {
  mode: RuntimeMode;
  environment: RuntimeEnvironment;
  kill_switch: boolean;
  daily_pnl: string;
  gross_exposure: string;
  net_exposure: string;
  open_alerts: number;
  daily_loss_limit: string;
  daily_loss_used: string;
  updated_at: string;
};

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

export type RiskAlertDto = ResourceVersion & {
  severity: AlertSeverity;
  reason: string;
  target: string;
  status: AlertStatus;
  created_at: string;
  updated_at: string;
};

export type RiskBucketDto = ResourceVersion & {
  name: string;
  exposure: string;
  limit: string;
  utilization: string;
  status: BucketStatus;
  updated_at: string;
};
