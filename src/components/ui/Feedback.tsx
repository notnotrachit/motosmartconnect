import type { ReactNode } from "react";
import { AnimatePresence, motion } from "motion/react";
import { cn } from "@/lib/cn";

/** Shimmering placeholder block. */
export function Skeleton({ className }: { className?: string }) {
  return <div aria-hidden="true" className={cn("skeleton rounded-md", className)} />;
}

/** Small numeric badge that pops when the count changes. Hidden at zero. */
export function CountBadge({ count, className, tone = "accent" }: { count: number; className?: string; tone?: "accent" | "muted" }) {
  return (
    <AnimatePresence initial={false}>
      {count > 0 && (
        <motion.span
          key={count > 99 ? "99+" : count}
          initial={{ scale: 0.5, opacity: 0 }}
          animate={{ scale: 1, opacity: 1 }}
          exit={{ scale: 0.5, opacity: 0 }}
          transition={{ type: "spring", stiffness: 520, damping: 22 }}
          className={cn(
            "inline-flex h-[18px] min-w-[18px] items-center justify-center rounded-full px-1.5 text-[10.5px] font-semibold tabular-nums leading-none",
            tone === "accent" ? "bg-accent text-on-accent" : "bg-surface-3 text-fg-muted",
            className,
          )}
        >
          {count > 99 ? "99+" : count}
        </motion.span>
      )}
    </AnimatePresence>
  );
}

export type DotTone = "live" | "idle" | "off" | "warn" | "error";

const DOT: Record<DotTone, string> = {
  live: "bg-success",
  idle: "bg-fg-subtle",
  off: "bg-fg-subtle/60",
  warn: "bg-warning",
  error: "bg-danger",
};

/** Status light with an optional radar ping. */
export function StatusDot({ tone, pulse = false, className }: { tone: DotTone; pulse?: boolean; className?: string }) {
  return (
    <span className={cn("relative inline-flex size-2 shrink-0", className)} aria-hidden="true">
      {pulse && <span className={cn("absolute inset-0 rounded-full opacity-60 animate-ping-slow", DOT[tone])} />}
      <span className={cn("relative inline-flex size-2 rounded-full transition-colors duration-300", DOT[tone])} />
    </span>
  );
}

interface EmptyStateProps {
  icon: ReactNode;
  title: string;
  description?: ReactNode;
  children?: ReactNode;
  className?: string;
  tone?: "default" | "error";
}

export function EmptyState({ icon, title, description, children, className, tone = "default" }: EmptyStateProps) {
  return (
    <motion.div
      initial={{ opacity: 0, y: 6 }}
      animate={{ opacity: 1, y: 0 }}
      transition={{ duration: 0.3, ease: [0.16, 1, 0.3, 1] }}
      className={cn("flex flex-col items-center justify-center px-6 py-14 text-center", className)}
    >
      <div
        className={cn(
          "mb-4 grid size-12 place-items-center rounded-2xl ring-1",
          tone === "error" ? "bg-danger/10 text-danger ring-danger/20" : "bg-surface-2 text-fg-muted ring-line",
        )}
      >
        {icon}
      </div>
      <h3 className="text-sm font-semibold text-fg">{title}</h3>
      {description && <p className="mt-1 max-w-[34ch] text-[0.8125rem] leading-relaxed text-fg-muted">{description}</p>}
      {children && <div className="mt-5 flex flex-wrap items-center justify-center gap-2">{children}</div>}
    </motion.div>
  );
}
