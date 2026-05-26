"use client";

import { useEffect } from "react";

import { sanitizeNextPath } from "@/lib/console-auth";

export default function LoginPage() {
  useEffect(() => {
    const searchParams = new URLSearchParams(window.location.search);
    window.location.replace(sanitizeNextPath(searchParams.get("next")));
  }, []);

  return null;
}
