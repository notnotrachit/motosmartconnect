import { useEffect, useRef } from "react";
import { AnimatePresence, motion } from "motion/react";
import { Lock, MousePointer2 } from "lucide-react";
import { mirror } from "@/engine/mirror";
import { cn } from "@/lib/cn";
import type { CrossView } from "@/lib/types";

/** The phone's screen as a trackpad: pointer and keys over it go to the phone. */
export function CrossPad({ cross }: { cross: CrossView }) {
  const pad = useRef<HTMLDivElement>(null);
  useEffect(() => {
    if (!pad.current) return;
    return mirror().mountPad(pad.current);
  }, []);

  const live = cross.active && cross.width > 0 && cross.height > 0;
  const rw = live ? cross.width : 16;
  const rh = live ? cross.height : 10;

  return (
    <div className="flex min-h-0 flex-1 items-center justify-center [container-type:size]">
      <div
        ref={pad}
        tabIndex={0}
        data-forward-keys
        role="application"
        aria-label="Phone pointer pad"
        className={cn(
          "relative overflow-hidden rounded-3xl border outline-none transition-[border-color,box-shadow] duration-300",
          "focus-visible:ring-2 focus-visible:ring-accent/60",
          cross.active
            ? "cursor-crosshair border-accent/50 bg-surface shadow-glow"
            : "cursor-not-allowed border-line bg-surface-2/60",
        )}
        style={{ aspectRatio: `${rw} / ${rh}`, width: `min(100cqw, calc(100cqh * ${rw} / ${rh}))` }}
      >
        <div
          aria-hidden="true"
          className={cn(
            "pointer-events-none absolute inset-0 transition-opacity duration-300 [background-image:radial-gradient(var(--line-strong)_1px,transparent_1px)] [background-size:18px_18px]",
            cross.active ? "opacity-100" : "opacity-50",
          )}
        />
        <AnimatePresence mode="wait" initial={false}>
          <motion.div
            key={cross.active ? "on" : "off"}
            initial={{ opacity: 0, scale: 0.96 }}
            animate={{ opacity: 1, scale: 1 }}
            exit={{ opacity: 0 }}
            className="pointer-events-none absolute inset-0 flex flex-col items-center justify-center gap-2 text-center"
          >
            {cross.active ? (
              <>
                <MousePointer2 size={22} className="text-accent" />
                <p className="text-xs font-medium text-fg-muted">Move here to point on the phone</p>
              </>
            ) : (
              <>
                <Lock size={20} className="text-fg-subtle" />
                <p className="text-xs text-fg-subtle">Start cross control to use the pad</p>
              </>
            )}
          </motion.div>
        </AnimatePresence>
      </div>
    </div>
  );
}
