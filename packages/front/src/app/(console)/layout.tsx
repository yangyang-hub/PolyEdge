import { ConsoleShell } from "@/components/shared/console-shell";

export const dynamic = "force-dynamic";
export const revalidate = 0;

export default function ConsoleLayout({
  children,
}: {
  children: React.ReactNode;
}) {
  return <ConsoleShell>{children}</ConsoleShell>;
}
