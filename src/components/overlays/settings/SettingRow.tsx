import type { ReactNode } from "react";
import { cn } from "@/lib/cn";

export function SettingGroup({ title, description, children }: { title: string; description?: string; children: ReactNode }) {
  return (
    <section className="mb-7 last:mb-0">
      <h3 className="text-[11px] font-semibold uppercase tracking-wider text-fg-subtle">{title}</h3>
      {description && <p className="mt-1 text-xs text-fg-muted">{description}</p>}
      <div className="mt-2.5 divide-y divide-line overflow-hidden rounded-xl border border-line bg-surface-2/40">{children}</div>
    </section>
  );
}

interface SettingRowProps {
  label: ReactNode;
  description?: ReactNode;
  children?: ReactNode;
  htmlFor?: string;
  className?: string;
}

export function SettingRow({ label, description, children, htmlFor, className }: SettingRowProps) {
  return (
    <div className={cn("flex items-center gap-4 px-4 py-3", className)}>
      <div className="min-w-0 flex-1">
        <label htmlFor={htmlFor} className="block text-[0.8125rem] font-medium text-fg">
          {label}
        </label>
        {description && <p className="mt-0.5 text-xs leading-relaxed text-fg-muted">{description}</p>}
      </div>
      {children && <div className="flex shrink-0 items-center gap-2">{children}</div>}
    </div>
  );
}
