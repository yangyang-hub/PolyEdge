import { ConsoleShell } from "@/components/shared/console-shell";

export default function ConsoleLayout({
  children,
}: {
  children: React.ReactNode;
}) {
  return <ConsoleShell>{children}</ConsoleShell>;
}
