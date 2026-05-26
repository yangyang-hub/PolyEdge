"use client";

import { Languages } from "lucide-react";

import { Button } from "@/components/ui/button";
import { useI18n } from "@/lib/i18n/client";
import { LOCALES, type Locale } from "@/lib/i18n/locales";
import { cn } from "@/lib/utils";

const localeButtonLabels: Record<Locale, string> = {
  "zh-CN": "中",
  "en-US": "EN",
};

export function LanguageSwitcher() {
  const { locale, dictionary, setLocale } = useI18n();

  function switchLocale(nextLocale: Locale) {
    if (nextLocale === locale) {
      return;
    }

    setLocale(nextLocale);
  }

  return (
    <div
      className="flex h-8 items-center gap-1 rounded-sm border border-white/10 bg-accent/35 px-1"
      aria-label={dictionary.common.language}
    >
      <Languages className="ml-1 size-3.5 text-muted-foreground" />
      {LOCALES.map((item) => {
        const active = item === locale;
        return (
          <Button
            key={item}
            type="button"
            variant="ghost"
            size="sm"
            onClick={() => switchLocale(item)}
            className={cn(
              "h-6 rounded-sm px-2 text-[11px] font-bold",
              active
                ? "bg-primary text-primary-foreground hover:bg-primary/90"
                : "text-muted-foreground hover:bg-accent hover:text-foreground",
            )}
          >
            {localeButtonLabels[item]}
          </Button>
        );
      })}
    </div>
  );
}
