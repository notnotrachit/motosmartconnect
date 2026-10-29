import type { ButtonHTMLAttributes, ReactNode, Ref } from "react";
import { cn } from "@/lib/cn";
import { Spinner } from "./Spinner";

export type ButtonVariant = "primary" | "secondary" | "outline" | "ghost" | "danger";
export type ButtonSize = "sm" | "md" | "lg";

const VARIANTS: Record<ButtonVariant, string> = {
  primary:
    "bg-accent text-on-accent hover:bg-accent-strong shadow-[inset_0_1px_0_rgb(255_255_255/0.18),0_1px_2px_rgb(0_0_0/0.18)]",
  secondary: "bg-surface-2 text-fg border border-line hover:bg-surface-3 hover:border-line-strong",
  outline: "border border-line-strong text-fg hover:bg-surface-2",
  ghost: "text-fg-muted hover:text-fg hover:bg-surface-2",
  danger: "bg-danger/10 text-danger border border-danger/20 hover:bg-danger/15 hover:border-danger/35",
};

const SIZES: Record<ButtonSize, string> = {
  sm: "h-7 px-2.5 text-xs gap-1.5 rounded-md",
  md: "h-8 px-3.5 text-[0.8125rem] gap-2 rounded-lg",
  lg: "h-10 px-5 text-sm gap-2 rounded-xl",
};

export interface ButtonProps extends ButtonHTMLAttributes<HTMLButtonElement> {
  variant?: ButtonVariant;
  size?: ButtonSize;
  /** Shows a spinner in place of the label and blocks clicks. */
  loading?: boolean;
  icon?: ReactNode;
  ref?: Ref<HTMLButtonElement>;
}

export function Button({
  variant = "secondary",
  size = "md",
  loading = false,
  icon,
  className,
  children,
  disabled,
  type = "button",
  ref,
  ...rest
}: ButtonProps) {
  return (
    <button
      ref={ref}
      type={type}
      disabled={disabled || loading}
      aria-busy={loading || undefined}
      className={cn(
        "relative inline-flex shrink-0 select-none items-center justify-center whitespace-nowrap font-medium",
        "transition-[background-color,border-color,color,box-shadow,transform,opacity] duration-150 ease-out",
        "active:scale-[0.97] disabled:pointer-events-none disabled:opacity-45",
        VARIANTS[variant],
        SIZES[size],
        className,
      )}
      {...rest}
    >
      <span className={cn("inline-flex items-center gap-[inherit] transition-opacity duration-150", loading && "opacity-0")}>
        {icon}
        {children}
      </span>
      {loading && (
        <span className="absolute inset-0 grid place-items-center">
          <Spinner size={14} />
        </span>
      )}
    </button>
  );
}
