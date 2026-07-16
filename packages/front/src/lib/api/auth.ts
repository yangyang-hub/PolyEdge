import { fetchContract, fetchWriteContract, randomUUID } from "@/lib/api/base";
import type { ApiResponse } from "@/lib/contracts/api";
import type { ActivateRequest, AuthSessionDto, CurrentUserDto, LoginRequest } from "@/lib/contracts/dto";

export function getCurrentUser(): Promise<ApiResponse<AuthSessionDto>> {
  return fetchContract("/api/v1/auth/me");
}

export function login(request: LoginRequest): Promise<ApiResponse<AuthSessionDto>> {
  return fetchWriteContract("/api/v1/auth/login", { body: request, idempotencyKey: randomUUID() });
}

export function activate(request: ActivateRequest): Promise<ApiResponse<CurrentUserDto>> {
  return fetchWriteContract("/api/v1/auth/activate", { body: request, idempotencyKey: randomUUID() });
}

export function logout(): Promise<void> {
  return fetchWriteContract("/api/v1/auth/logout", { body: {}, idempotencyKey: randomUUID() });
}
