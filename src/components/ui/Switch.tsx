import { cn } from "@/lib/cn";

interface SwitchProps {
  checked: boolean;
  onChange: (checked: boolean) => void;
  label: string;
  disabled?: boolean;
  id?: string;
}

export function Switch({ checked, onChange, label, disabled, id }: SwitchProps) {
  return (
    <button
      id={id}
      type="button"
      role="switch"
      aria-checked={checked}
      aria-label={label}
      disabled={disabled}
      onClick={() => onChange(!checked)}
      className={cn(
        "relative inline-flex h-[22px] w-[38px] shrink-0 items-center rounded-full p-[2px] transition-colors duration-200",
        "disabled:opacity-40",
        checked ? "bg-accent" : "bg-surface-3 ring-1 ring-inset ring-line-strong",
      )}
    >
      <span
        className={cn(
          "size-[18px] rounded-full bg-white shadow-[0_1px_3px_rgb(0_0_0/0.3)] transition-transform duration-300 ease-spring",
          checked && "translate-x-4",
        )}
      />
    </button>
  );
}
