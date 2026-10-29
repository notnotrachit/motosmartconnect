import type { ReactNode } from "react";
import { motion } from "motion/react";
import { cn } from "@/lib/cn";

export interface SegmentOption<T extends string> {
  value: T;
  label: ReactNode;
  icon?: ReactNode;
}

interface SegmentedProps<T extends string> {
  /** Unique per instance; drives the shared-layout highlight. */
  id: string;
  value: T;
  options: SegmentOption<T>[];
  onChange: (value: T) => void;
  label: string;
  className?: string;
}

export function Segmented<T extends string>({ id, value, options, onChange, label, className }: SegmentedProps<T>) {
  return (
    <div role="radiogroup" aria-label={label} className={cn("inline-flex rounded-lg border border-line bg-surface-2 p-0.5", className)}>
      {options.map((option) => {
        const selected = option.value === value;
        return (
          <button
            key={option.value}
            type="button"
            role="radio"
            aria-checked={selected}
            onClick={() => onChange(option.value)}
            className={cn(
              "relative h-7 flex-1 rounded-md px-3 text-xs font-medium transition-colors duration-150",
              selected ? "text-fg" : "text-fg-muted hover:text-fg",
            )}
          >
            {selected && (
              <motion.span
                layoutId={`segmented-${id}`}
                className="absolute inset-0 rounded-md bg-surface shadow-card ring-1 ring-line dark:bg-surface-3"
                transition={{ type: "spring", bounce: 0.18, duration: 0.4 }}
              />
            )}
            <span className="relative z-10 inline-flex items-center justify-center gap-1.5">
              {option.icon}
              {option.label}
            </span>
          </button>
        );
      })}
    </div>
  );
}
