import { Inbox } from "lucide-react";

export function EmptyPanel({
  title,
  detail,
}: {
  title: string;
  detail: string;
}) {
  return (
    <div className="flex flex-col items-center justify-center rounded-lg bg-card/95 px-6 py-12 text-center ring-1 ring-white/5">
      <div className="flex size-10 items-center justify-center rounded-full bg-accent/60 text-primary">
        <Inbox className="size-5" />
      </div>
      <p className="mt-4 font-heading text-lg font-bold text-foreground">{title}</p>
      <p className="mt-2 max-w-md text-sm text-muted-foreground">{detail}</p>
    </div>
  );
}
