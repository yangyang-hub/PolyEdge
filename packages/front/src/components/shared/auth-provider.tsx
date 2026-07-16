"use client";

import { createContext, useContext, useEffect, useMemo, useState, type ReactNode } from "react";
import { usePathname } from "next/navigation";

import { getCurrentUser, logout } from "@/lib/api/auth";
import type { CurrentUserDto, UserRole } from "@/lib/contracts/dto";

type AuthState = { user: CurrentUserDto | null; loading: boolean; signOut: () => Promise<void> };
const AuthContext = createContext<AuthState>({ user: null, loading: true, signOut: async () => undefined });

export function AuthProvider({ children }: { children: ReactNode }) {
  const pathname = usePathname();
  const [user, setUser] = useState<CurrentUserDto | null>(null);
  const [loading, setLoading] = useState(true);

  useEffect(() => {
    let active = true;
    void getCurrentUser().then((response) => {
      if (!active) return;
      const current = response.data.user;
      setUser(current);
      if ((pathname.startsWith("/admin/") || pathname === "/settings") && current.role !== "admin") window.location.replace("/unauthorized");
    }).catch(() => {
      if (active) window.location.replace(`/login?next=${encodeURIComponent(pathname)}`);
    }).finally(() => active && setLoading(false));
    return () => { active = false; };
  }, [pathname]);

  const value = useMemo<AuthState>(() => ({
    user,
    loading,
    signOut: async () => { await logout(); window.location.replace("/login"); },
  }), [loading, user]);

  return <AuthContext.Provider value={value}>{children}</AuthContext.Provider>;
}

export function useAuth() { return useContext(AuthContext); }
export function canWriteMarkets(role: UserRole | undefined) { return role === "admin" || role === "market_editor"; }
export function isAdmin(role: UserRole | undefined) { return role === "admin"; }
