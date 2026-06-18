"use client";

import { useEffect, useRef, useState } from "react";

import { Tooltip, TooltipContent, TooltipTrigger } from "@/components/ui/tooltip";
import { cn } from "@/lib/utils";

// 字面量映射，确保 Tailwind v4 扫描到这些 class 并生成（避免动态拼接被 purge）。
const LINE_CLAMP_CLASS = {
  1: "line-clamp-1",
  2: "line-clamp-2",
  3: "line-clamp-3",
  4: "line-clamp-4",
  5: "line-clamp-5",
} as const;

/**
 * 多行截断长文本；**仅当内容实际溢出时**才在 hover/focus 时用 Tooltip 显示完整文本，
 * 避免对短文本也弹浮框。line-clamp 只是视觉截断，DOM 内仍保留完整文本，屏幕阅读器可读全文。
 */
export function TruncateText({
  text,
  lines = 2,
  className,
}: {
  text: string;
  lines?: 1 | 2 | 3 | 4 | 5;
  className?: string;
}) {
  const ref = useRef<HTMLSpanElement>(null);
  const [overflowing, setOverflowing] = useState(false);

  useEffect(() => {
    const el = ref.current;
    if (!el) return;
    setOverflowing(el.scrollHeight > el.clientHeight + 1);
  }, [text, lines, className]);

  const clampClass = LINE_CLAMP_CLASS[lines];

  const content = (
    <span ref={ref} className={cn(clampClass, className)}>
      {text}
    </span>
  );

  if (!overflowing) {
    return content;
  }

  return (
    <Tooltip>
      <TooltipTrigger asChild>{content}</TooltipTrigger>
      <TooltipContent side="top" className="max-w-sm whitespace-pre-wrap text-wrap">
        {text}
      </TooltipContent>
    </Tooltip>
  );
}
