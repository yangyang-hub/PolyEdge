import { ConsoleShell } from "@/components/shared/console-shell";
import { AuthProvider } from "@/components/shared/auth-provider";

export default function ConsoleLayout({
  children,
}: {
  children: React.ReactNode;
}) {
  return <AuthProvider><ConsoleShell>{children}</ConsoleShell></AuthProvider>;
}
