import { Link2 } from "lucide-react";
import { cn } from "@/lib/cn";

export function BrandMark({ size = 22, className }: { size?: number; className?: string }) {
  return (
    <span
      data-tauri-drag-region
      aria-hidden="true"
      className={cn(
        "grid shrink-0 place-items-center rounded-[7px] bg-linear-to-br from-accent to-accent-strong text-on-accent",
        "shadow-[inset_0_1px_0_rgb(255_255_255/0.25),0_2px_6px_-1px_color-mix(in_srgb,var(--accent)_60%,transparent)]",
        className,
      )}
      style={{ width: size, height: size }}
    >
      <Link2 size={size * 0.6} strokeWidth={2.4} data-tauri-drag-region />
    </span>
  );
}

export function Brand() {
  return (
    <div data-tauri-drag-region className="flex items-center gap-2">
      <BrandMark />
      <span data-tauri-drag-region className="font-display text-[0.8125rem] font-semibold tracking-tight text-fg">
        Smart Connect
      </span>
    </div>
  );
}
