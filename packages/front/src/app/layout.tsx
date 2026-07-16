import type { Metadata } from "next";

import { TooltipProvider } from "@/components/ui/tooltip";
import { dictionary } from "@/lib/i18n/dictionaries";
import "./globals.css";

export const metadata: Metadata = {
  title: dictionary.meta.title,
  description: dictionary.meta.description,
};

export default function RootLayout({
  children,
}: Readonly<{
  children: React.ReactNode;
}>) {
  return (
    <html lang="zh-CN" className="dark h-full antialiased">
      <body className="min-h-full bg-background text-foreground">
        <TooltipProvider delayDuration={150}>
          {children}
        </TooltipProvider>
      </body>
    </html>
  );
}
