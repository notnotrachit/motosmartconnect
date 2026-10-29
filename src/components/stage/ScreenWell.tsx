import { useEffect, useRef } from "react";
import { AnimatePresence, motion } from "motion/react";
import { MousePointerClick } from "lucide-react";
import { mirror } from "@/engine/mirror";
import { useScreen } from "@/hooks/useScreen";
import { cn } from "@/lib/cn";
import { useConnectedPhone, useWallpaper } from "@/store/engine";
import { ScreenOverlay } from "./ScreenOverlay";
import { screenPhase } from "./screenPhase";

/**
 * The phone-shaped surface. The controller owns the <video>/<img> and mounts
 * them into `host`, which React never renders into, so re-renders cannot
 * disturb a playing picture.
 */
export function ScreenWell() {
  const well = useRef<HTMLDivElement>(null);
  const host = useRef<HTMLDivElement>(null);
  const screen = useScreen();
  const connected = Boolean(useConnectedPhone());
  const wallpaper = useWallpaper();
  const phase = screenPhase(screen, connected);

  useEffect(() => {
    if (!well.current || !host.current) return;
    return mirror().mountScreen(well.current, host.current);
  }, []);

  // Size the well to the picture. Portrait phone until the first frame says otherwise.
  const rw = screen.width || 9;
  const rh = screen.height || 19.5;

  return (
    <div
      ref={well}
      tabIndex={0}
      data-forward-keys
      aria-label="Phone screen. Click to tap; keys you type go to the phone."
      className={cn(
        "group/well relative overflow-hidden rounded-[32px] bg-[#07080a] outline-none",
        "ring-1 ring-black/10 transition-[box-shadow] duration-500 dark:ring-white/10",
        "shadow-[0_30px_60px_-20px_rgb(0_0_0/0.45),inset_0_0_0_6px_#121418,inset_0_0_0_7px_rgb(255_255_255/0.06)]",
        "focus-visible:ring-2 focus-visible:ring-accent",
        phase === "live" && "shadow-[0_30px_60px_-20px_rgb(0_0_0/0.45),0_0_0_2px_color-mix(in_srgb,var(--accent)_70%,transparent),0_0_40px_-4px_color-mix(in_srgb,var(--accent)_35%,transparent)]",
        phase === "live" ? "cursor-pointer" : "cursor-default",
      )}
      style={{
        aspectRatio: `${rw} / ${rh}`,
        width: `min(100cqw, calc(100cqh * ${rw} / ${rh}))`,
      }}
    >
      <div
        ref={host}
        className={cn(
          "absolute inset-[7px] overflow-hidden rounded-[26px] transition-opacity duration-300",
          // Opacity only (no layout change), so a playing picture is never disturbed.
          phase === "live" ? "opacity-100" : "opacity-0",
        )}
      />

      {phase !== "live" && wallpaper && (
        <img
          src={wallpaper}
          alt=""
          className="pointer-events-none absolute inset-[7px] h-[calc(100%-14px)] w-[calc(100%-14px)] rounded-[26px] object-cover"
        />
      )}
      {phase !== "live" && (
        <div
          aria-hidden="true"
          className={
            wallpaper
              ? "pointer-events-none absolute inset-[7px] rounded-[26px] bg-black/35"
              : "pointer-events-none absolute inset-[7px] rounded-[26px] bg-[radial-gradient(120%_70%_at_50%_0%,color-mix(in_srgb,var(--accent)_16%,transparent),transparent_60%)]"
          }
        />
      )}

      <ScreenOverlay phase={phase} label={screen.label} />

      <AnimatePresence>
        {phase === "live" && (
          <motion.div
            initial={{ opacity: 0, y: -6 }}
            animate={{ opacity: 1, y: 0 }}
            exit={{ opacity: 0 }}
            className="pointer-events-none absolute inset-x-0 top-4 z-[4] flex justify-center"
          >
            <span className="flex items-center gap-1.5 rounded-full bg-black/55 px-2.5 py-1 text-[10.5px] font-semibold uppercase tracking-wider text-white backdrop-blur-md transition-opacity duration-300 group-hover/well:opacity-0">
              <span className="size-1.5 animate-pulse rounded-full bg-red-500" />
              Live
            </span>
          </motion.div>
        )}
      </AnimatePresence>

      {phase === "live" && (
        <div className="pointer-events-none absolute inset-x-0 bottom-4 z-[4] flex justify-center opacity-0 transition-opacity delay-700 duration-300 group-hover/well:opacity-100">
          <span className="flex items-center gap-1.5 rounded-full bg-black/60 px-2.5 py-1 text-[10.5px] text-white/85 backdrop-blur-md">
            <MousePointerClick size={11} />
            Click to tap · scroll to swipe · type to send keys
          </span>
        </div>
      )}
    </div>
  );
}
