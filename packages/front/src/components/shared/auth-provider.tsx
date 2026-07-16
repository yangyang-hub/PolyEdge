"use client";

import { createContext, useContext, useEffect, useMemo, useState, type ReactNode } from "react";
import { usePathname } from "next/navigation";

import { ConsoleLoadingSkeleton } from "@/components/shared/console-loading-skeleton";
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
    }).catch(() => {
      if (active) window.location.replace(`/login?next=${encodeURIComponent(window.location.pathname)}`);
    }).finally(() => active && setLoading(false));
    return () => { active = false; };
  }, []);

  const requiresAdmin = pathname.startsWith("/admin/") || pathname === "/settings";
  const authorized = !requiresAdmin || user?.role === "admin";

  useEffect(() => {
    if (loading || !user || authorized) return;
    const query = new URLSearchParams({
      next: pathname,
      required: "admin",
      current: user.role,
    });
    window.location.replace(`/unauthorized?${query.toString()}`);
  }, [authorized, loading, pathname, user]);

  const value = useMemo<AuthState>(() => ({
    user,
    loading,
    signOut: async () => { await logout(); window.location.replace("/login"); },
  }), [loading, user]);

  return (
    <AuthContext.Provider value={value}>
      {loading || !user || !authorized ? <ConsoleLoadingSkeleton /> : children}
    </AuthContext.Provider>
  );
}

export function useAuth() { return useContext(AuthContext); }
export function canWriteMarkets(role: UserRole | undefined) { return role === "admin" || role === "market_editor"; }
