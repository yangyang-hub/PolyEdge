"use client";

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
 * 多行截断长文本，hover/focus 时用 Tooltip 显示完整文本。
 *
 * 不做「仅溢出才弹浮框」的判定——因为 `display:-webkit-box` + `-webkit-line-clamp`
 * 元素的 `scrollHeight` 在浏览器里有已知 quirk（常等于 clientHeight），无法可靠检测
 * 是否被截断。改为始终挂 Tooltip：Radix Tooltip 全局设了 150ms delay，快速划过不弹，
 * 只有 hover 停留才显示；短文本弹相同内容略冗余但无害，长文本一定能看到全文。
 *
 * line-clamp 只是视觉截断，DOM 仍保留完整文本，屏幕阅读器可读全文。
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
  return (
    <Tooltip>
      <TooltipTrigger asChild>
        <span className={cn(LINE_CLAMP_CLASS[lines], className)}>{text}</span>
      </TooltipTrigger>
      <TooltipContent side="top" className="max-w-sm whitespace-pre-wrap text-wrap">
        {text}
      </TooltipContent>
    </Tooltip>
  );
}
