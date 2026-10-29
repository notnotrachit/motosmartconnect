import type { ButtonHTMLAttributes, ReactNode, Ref } from "react";
import { cn } from "@/lib/cn";
import { Spinner } from "./Spinner";
import { Tooltip } from "./Tooltip";

interface IconButtonProps extends Omit<ButtonHTMLAttributes<HTMLButtonElement>, "children"> {
  /** Accessible name; also shown as the tooltip. */
  label: string;
  icon: ReactNode;
  shortcut?: string;
  tooltipSide?: "top" | "bottom" | "left" | "right";
  size?: "sm" | "md" | "lg";
  variant?: "ghost" | "solid" | "accent" | "danger";
  active?: boolean;
  loading?: boolean;
  noTooltip?: boolean;
  ref?: Ref<HTMLButtonElement>;
}

const SIZES = { sm: "size-7 rounded-md", md: "size-8 rounded-lg", lg: "size-10 rounded-xl" };
const VARIANTS = {
  ghost: "text-fg-muted hover:bg-surface-2 hover:text-fg",
  solid: "border border-line bg-surface-2 text-fg hover:bg-surface-3",
  accent: "bg-accent text-on-accent hover:bg-accent-strong",
  danger: "text-fg-muted hover:bg-danger/10 hover:text-danger",
};

export function IconButton({
  label,
  icon,
  shortcut,
  tooltipSide = "bottom",
  size = "md",
  variant = "ghost",
  active = false,
  loading = false,
  noTooltip = false,
  className,
  disabled,
  type = "button",
  ref,
  ...rest
}: IconButtonProps) {
  const button = (
    <button
      ref={ref}
      type={type}
      aria-label={label}
      aria-pressed={active || undefined}
      disabled={disabled || loading}
      className={cn(
        "inline-flex shrink-0 items-center justify-center transition-[background-color,color,transform] duration-150",
        "active:scale-90 disabled:pointer-events-none disabled:opacity-40",
        SIZES[size],
        VARIANTS[variant],
        active && "bg-accent/15 text-accent hover:bg-accent/20 hover:text-accent",
        className,
      )}
      {...rest}
    >
      {loading ? <Spinner size={14} /> : icon}
    </button>
  );
  if (noTooltip) return button;
  return (
    <Tooltip label={label} shortcut={shortcut} side={tooltipSide}>
      {button}
    </Tooltip>
  );
}
