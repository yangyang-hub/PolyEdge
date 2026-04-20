export function ConsoleLoadingSkeleton() {
  return (
    <div className="space-y-6 animate-pulse" aria-hidden="true">
      <div className="space-y-3">
        <div className="h-3 w-24 rounded-full bg-accent" />
        <div className="h-10 w-64 rounded-full bg-accent/80" />
        <div className="h-4 w-full max-w-2xl rounded-full bg-accent/60" />
      </div>

      <div className="grid gap-4 md:grid-cols-2 xl:grid-cols-4">
        {Array.from({ length: 4 }).map((_, index) => (
          <div key={index} className="rounded-2xl border border-border/60 bg-card/95 p-5">
            <div className="h-3 w-20 rounded-full bg-accent" />
            <div className="mt-4 h-8 w-28 rounded-full bg-accent/80" />
            <div className="mt-3 h-3 w-24 rounded-full bg-accent/60" />
          </div>
        ))}
      </div>

      <div className="grid gap-4 xl:grid-cols-[1.55fr_0.95fr]">
        <div className="rounded-2xl border border-border/60 bg-card/95 p-5">
          <div className="h-4 w-40 rounded-full bg-accent" />
          <div className="mt-5 space-y-3">
            {Array.from({ length: 5 }).map((_, index) => (
              <div key={index} className="rounded-xl bg-accent/40 p-4">
                <div className="h-3 w-32 rounded-full bg-accent" />
                <div className="mt-3 h-4 w-full rounded-full bg-accent/70" />
                <div className="mt-2 h-4 w-2/3 rounded-full bg-accent/55" />
              </div>
            ))}
          </div>
        </div>

        <div className="space-y-4">
          {Array.from({ length: 2 }).map((_, index) => (
            <div key={index} className="rounded-2xl border border-border/60 bg-card/95 p-5">
              <div className="h-4 w-32 rounded-full bg-accent" />
              <div className="mt-5 space-y-3">
                {Array.from({ length: 3 }).map((__, innerIndex) => (
                  <div key={innerIndex} className="rounded-xl bg-accent/40 p-4">
                    <div className="h-3 w-24 rounded-full bg-accent" />
                    <div className="mt-3 h-4 w-full rounded-full bg-accent/70" />
                  </div>
                ))}
              </div>
            </div>
          ))}
        </div>
      </div>
    </div>
  );
}
