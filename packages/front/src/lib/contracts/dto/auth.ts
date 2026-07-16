export type UserRole = "admin" | "market_editor" | "read_only";
export type UserStatus = "pending" | "active" | "disabled" | "locked";

export type CurrentUserDto = {
  id: number;
  username: string;
  display_name: string;
  role: UserRole;
  status: UserStatus;
  auth_source: "environment_admin" | "local";
  created_by_user_id: number | null;
  credential_version: number;
  created_at: string;
  updated_at: string;
};

export type AuthSessionDto = { user: CurrentUserDto; csrf_token?: string };
export type LoginRequest = { username: string; password: string };
export type ActivateRequest = { token: string; password: string };

export type AdminUserDto = CurrentUserDto;

export type AdminFinanceDto = {
  user_id: number;
  username: string;
  display_name: string;
  wallet_count: number;
  equity: string;
  available_collateral: string;
  realized_pnl: string;
  unrealized_pnl: string;
  total_pnl: string;
  valuation_complete: boolean;
  reserved_collateral: string;
  position_market_value: string;
  observed_at: string | null;
};
