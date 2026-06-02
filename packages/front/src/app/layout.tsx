import type { Metadata } from "next";
import { Inter, Manrope, Roboto_Mono } from "next/font/google";

import { TooltipProvider } from "@/components/ui/tooltip";
import { dictionary } from "@/lib/i18n/dictionaries";
import { Toaster } from "sonner";
import "./globals.css";

const inter = Inter({
  variable: "--font-inter",
  subsets: ["latin"],
});

const manrope = Manrope({
  variable: "--font-manrope",
  subsets: ["latin"],
});

const robotoMono = Roboto_Mono({
  variable: "--font-roboto-mono",
  subsets: ["latin"],
});

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
    <html
      lang="zh-CN"
      className={`${inter.variable} ${manrope.variable} ${robotoMono.variable} dark h-full antialiased`}
    >
      <body className="min-h-full bg-background text-foreground">
        <TooltipProvider delayDuration={150}>
          {children}
          <Toaster richColors position="top-right" theme="dark" />
        </TooltipProvider>
      </body>
    </html>
  );
}
