export type SystemRuntimeStateData = {
  kill_switch_locked: boolean;
  trading_enabled: boolean;
  reason: string | null;
  version: number;
  updated_by: string;
  updated_at: string;
};

export type UpdateSystemRuntimeStateRequest = {
  kill_switch_locked: boolean;
  trading_enabled: boolean;
  reason?: string;
  operator_note?: string;
};
