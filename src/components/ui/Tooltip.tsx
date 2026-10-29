import type { ReactNode } from "react";
import { cn } from "@/lib/cn";
import { Kbd } from "./Kbd";

interface TooltipProps {
  label: ReactNode;
  shortcut?: string;
  side?: "top" | "bottom" | "left" | "right";
  children: ReactNode;
  className?: string;
}

const SIDES = {
  top: "bottom-full left-1/2 mb-2 -translate-x-1/2 translate-y-1 group-hover/tip:translate-y-0",
  bottom: "top-full left-1/2 mt-2 -translate-x-1/2 -translate-y-1 group-hover/tip:translate-y-0",
  left: "right-full top-1/2 mr-2 -translate-y-1/2 translate-x-1 group-hover/tip:translate-x-0",
  right: "left-full top-1/2 ml-2 -translate-y-1/2 -translate-x-1 group-hover/tip:translate-x-0",
};

/** CSS-only tooltip: appears after a short hover delay or on keyboard focus. */
export function Tooltip({ label, shortcut, side = "bottom", children, className }: TooltipProps) {
  return (
    <span className={cn("group/tip relative inline-flex", className)}>
      {children}
      <span
        role="tooltip"
        className={cn(
          "pointer-events-none absolute z-[60] flex items-center gap-1.5 whitespace-nowrap rounded-md border border-line",
          "bg-glass px-2 py-1 text-[11px] font-medium text-fg shadow-pop backdrop-blur-xl",
          "opacity-0 transition-[opacity,translate] duration-150 ease-out",
          "group-hover/tip:opacity-100 group-hover/tip:delay-500 group-has-[:focus-visible]/tip:opacity-100",
          SIDES[side],
        )}
      >
        {label}
        {shortcut && <Kbd>{shortcut}</Kbd>}
      </span>
    </span>
  );
}
