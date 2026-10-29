import type { ReactNode } from "react";

interface PanelHeaderProps {
  title: string;
  subtitle?: ReactNode;
  children?: ReactNode;
}

/** Title row of a panel with right-aligned tools. */
export function PanelHeader({ title, subtitle, children }: PanelHeaderProps) {
  return (
    <div className="flex shrink-0 items-center gap-3 px-6 pb-3 pt-5">
      <div className="min-w-0 flex-1">
        <h2 className="font-display text-[1.05rem] font-semibold tracking-tight text-fg">{title}</h2>
        {subtitle && <p className="mt-0.5 truncate text-xs text-fg-muted">{subtitle}</p>}
      </div>
      {children && <div className="flex shrink-0 items-center gap-2">{children}</div>}
    </div>
  );
}
