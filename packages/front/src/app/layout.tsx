import type { Metadata } from "next";
import { Inter, Manrope, Roboto_Mono } from "next/font/google";

import { TooltipProvider } from "@/components/ui/tooltip";
import { I18nProvider } from "@/lib/i18n/client";
import { getServerI18n } from "@/lib/i18n/server";
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

export async function generateMetadata(): Promise<Metadata> {
  const { dictionary } = await getServerI18n();

  return {
    title: dictionary.meta.title,
    description: dictionary.meta.description,
  };
}

export default async function RootLayout({
  children,
}: Readonly<{
  children: React.ReactNode;
}>) {
  const { locale, dictionary } = await getServerI18n();

  return (
    <html
      lang={locale}
      className={`${inter.variable} ${manrope.variable} ${robotoMono.variable} dark h-full antialiased`}
    >
      <body className="min-h-full bg-background text-foreground">
        <I18nProvider locale={locale} dictionary={dictionary}>
          <TooltipProvider delayDuration={150}>
            {children}
            <Toaster richColors position="top-right" theme="dark" />
          </TooltipProvider>
        </I18nProvider>
      </body>
    </html>
  );
}
