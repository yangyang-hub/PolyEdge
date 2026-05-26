import type { Metadata } from "next";
import { Inter, Manrope, Roboto_Mono } from "next/font/google";

import { TooltipProvider } from "@/components/ui/tooltip";
import { I18nProvider } from "@/lib/i18n/client";
import { DEFAULT_LOCALE } from "@/lib/i18n/locales";
import { createI18nRuntime } from "@/lib/i18n/runtime";
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

const initialI18n = createI18nRuntime(DEFAULT_LOCALE);

export const metadata: Metadata = {
  title: initialI18n.dictionary.meta.title,
  description: initialI18n.dictionary.meta.description,
};

export default function RootLayout({
  children,
}: Readonly<{
  children: React.ReactNode;
}>) {
  return (
    <html
      lang={initialI18n.locale}
      className={`${inter.variable} ${manrope.variable} ${robotoMono.variable} dark h-full antialiased`}
    >
      <body className="min-h-full bg-background text-foreground">
        <I18nProvider locale={initialI18n.locale} dictionary={initialI18n.dictionary}>
          <TooltipProvider delayDuration={150}>
            {children}
            <Toaster richColors position="top-right" theme="dark" />
          </TooltipProvider>
        </I18nProvider>
      </body>
    </html>
  );
}
