import { fetchListContract, fetchWriteContract, randomUUID } from "@/lib/api/base";
import type { AdminFinanceDto, AdminUserDto, UserRole, UserStatus } from "@/lib/contracts/dto";
import type { ApiResponse } from "@/lib/contracts/api";

export const listAdminUsers = () => fetchListContract<AdminUserDto>("/api/v1/admin/users");
export const listAdminFinance = () => fetchListContract<AdminFinanceDto>("/api/v1/admin/finance");
export const createUser = (body: { username: string; display_name: string; role: UserRole }) =>
  fetchWriteContract<ApiResponse<{ user: AdminUserDto; activation_token: string; activation_expires_at: string }>>("/api/v1/admin/users", { body, idempotencyKey: randomUUID() });
export const reissueActivationToken = (userId: number) =>
  fetchWriteContract<ApiResponse<{ user_id: number; activation_token: string; activation_expires_at: string }>>(`/api/v1/admin/users/${userId}/activation-token`, { body: {}, idempotencyKey: randomUUID() });
export const updateAdminUser = (userId: number, body: { role?: UserRole; status?: UserStatus; display_name?: string }) =>
  fetchWriteContract<ApiResponse<AdminUserDto>>(`/api/v1/admin/users/${userId}`, { method: "PATCH", body, idempotencyKey: randomUUID() });
