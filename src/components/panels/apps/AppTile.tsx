import { useState } from "react";
import { motion } from "motion/react";
import { AppIcon } from "@/components/ui/AppIcon";
import { Spinner } from "@/components/ui/Spinner";
import { openApp } from "@/engine/actions";
import { cn } from "@/lib/cn";
import type { AppView } from "@/lib/types";

/** Launcher tile. Opening streams the app into its own window. */
export function AppTile({ app, icon, index }: { app: AppView; icon: string; index: number }) {
  const [opening, setOpening] = useState(false);
  const name = app.name || app.pkg;

  const launch = async () => {
    if (opening) return;
    setOpening(true);
    await openApp(app);
    setOpening(false);
  };

  return (
    <motion.li
      initial={{ opacity: 0, y: 8, scale: 0.96 }}
      animate={{ opacity: 1, y: 0, scale: 1 }}
      transition={{ delay: Math.min(index, 30) * 0.012, type: "spring", duration: 0.4, bounce: 0.2 }}
    >
      <button
        type="button"
        onClick={launch}
        title={app.pkg}
        className={cn(
          "group flex w-full flex-col items-center gap-2 rounded-2xl px-1.5 pb-2.5 pt-3 text-center outline-none",
          "transition-colors duration-150 hover:bg-surface-2 focus-visible:bg-surface-2 focus-visible:ring-2 focus-visible:ring-accent/50",
        )}
      >
        <span className="relative transition-transform duration-300 ease-spring group-hover:-translate-y-0.5 group-hover:scale-[1.06] group-active:scale-95">
          <AppIcon src={icon} name={name} size={48} className="shadow-[0_6px_14px_-6px_rgb(0_0_0/0.35)]" />
          {opening && (
            <span className="absolute inset-0 grid place-items-center rounded-[12px] bg-black/45 text-white">
              <Spinner size={16} />
            </span>
          )}
        </span>
        <span className="line-clamp-2 w-full text-[11.5px] font-medium leading-tight text-fg-muted transition-colors group-hover:text-fg">
          {name}
        </span>
      </button>
    </motion.li>
  );
}
