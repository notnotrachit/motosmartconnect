import type { InputHTMLAttributes, ReactNode, Ref } from "react";
import { X } from "lucide-react";
import { cn } from "@/lib/cn";

interface InputProps extends Omit<InputHTMLAttributes<HTMLInputElement>, "size"> {
  leading?: ReactNode;
  trailing?: ReactNode;
  /** Shows a clear button when there is a value. */
  onClear?: () => void;
  size?: "sm" | "md" | "lg";
  wrapperClassName?: string;
  ref?: Ref<HTMLInputElement>;
}

const SIZES = { sm: "h-7 text-xs rounded-md", md: "h-8 text-[0.8125rem] rounded-lg", lg: "h-10 text-sm rounded-xl" };

export function Input({
  leading,
  trailing,
  onClear,
  size = "md",
  className,
  wrapperClassName,
  value,
  ref,
  ...rest
}: InputProps) {
  const hasValue = value != null && String(value).length > 0;
  return (
    <div
      className={cn(
        "group/input relative flex items-center border border-line bg-surface-2 text-fg transition-[border-color,box-shadow,background-color] duration-150",
        "hover:border-line-strong focus-within:border-accent/60 focus-within:bg-surface focus-within:ring-[3px] focus-within:ring-accent/15",
        SIZES[size],
        wrapperClassName,
      )}
    >
      {leading && <span className="pointer-events-none flex pl-2.5 text-fg-subtle group-focus-within/input:text-fg-muted">{leading}</span>}
      <input
        ref={ref}
        value={value}
        className={cn(
          "h-full min-w-0 flex-1 bg-transparent px-2.5 outline-none placeholder:text-fg-subtle focus-visible:outline-none",
          leading && "pl-2",
          className,
        )}
        {...rest}
      />
      {onClear && hasValue && (
        <button
          type="button"
          aria-label="Clear"
          onClick={onClear}
          className="mr-1 grid size-5 place-items-center rounded text-fg-subtle transition hover:bg-surface-3 hover:text-fg"
        >
          <X size={12} />
        </button>
      )}
      {trailing && <span className="flex pr-1.5">{trailing}</span>}
    </div>
  );
}
