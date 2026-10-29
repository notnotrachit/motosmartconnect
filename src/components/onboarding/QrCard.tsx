import { useState } from "react";
import { AnimatePresence, motion } from "motion/react";
import { Clock3, QrCode, RefreshCw } from "lucide-react";
import { Button } from "@/components/ui/Button";
import { StatusDot } from "@/components/ui/Feedback";
import { Spinner } from "@/components/ui/Spinner";
import { Tooltip } from "@/components/ui/Tooltip";
import { refreshCode } from "@/engine/actions";
import { useNow } from "@/hooks/useNow";
import { cn } from "@/lib/cn";
import { formatCountdown } from "@/lib/format";
import { useEngine } from "@/store/engine";

type CodeState = "preparing" | "off" | "expired" | "ready";

/** The pairing QR code with its countdown and refresh control. */
export function QrCard({ className }: { className?: string }) {
  const snapshot = useEngine((s) => s.snapshot);
  const deadline = useEngine((s) => s.qrDeadline);
  const lifetime = useEngine((s) => s.qrLifetime);
  const now = useNow(1000);
  const [refreshing, setRefreshing] = useState(false);

  const svg = snapshot?.qrSvg ?? "";
  const seconds = Math.max(0, Math.round((deadline - now) / 1000));
  const state: CodeState = !snapshot?.listening
    ? "preparing"
    : !snapshot.pairing
      ? "off"
      : seconds <= 0 || !svg.includes("<svg")
        ? "expired"
        : "ready";
  const progress = lifetime ? Math.min(1, seconds / lifetime) : 0;
  const urgent = state === "ready" && seconds <= 15;

  const refresh = async () => {
    setRefreshing(true);
    await refreshCode();
    setRefreshing(false);
  };

  return (
    <div className={cn("rounded-[28px] border border-line bg-surface p-5 shadow-pop", className)}>
      <div className="relative mx-auto aspect-square w-full overflow-hidden rounded-2xl bg-white p-4 ring-1 ring-black/5">
        <Corners active={state === "ready"} />
        <AnimatePresence mode="popLayout" initial={false}>
          {svg.includes("<svg") && (
            <motion.div
              key={svg}
              initial={{ opacity: 0, scale: 0.94, filter: "blur(6px)" }}
              animate={{
                opacity: state === "ready" ? 1 : 0.18,
                scale: 1,
                filter: state === "ready" ? "blur(0px)" : "blur(3px)",
              }}
              exit={{ opacity: 0, scale: 1.03 }}
              transition={{ duration: 0.4, ease: [0.16, 1, 0.3, 1] }}
              className="qr-plate"
              role="img"
              aria-label="Pairing QR code"
              // The SVG is generated locally by the Rust engine (qrcode crate), not user content.
              dangerouslySetInnerHTML={{ __html: svg }}
            />
          )}
        </AnimatePresence>

        <AnimatePresence>
          {state !== "ready" && (
            <motion.div
              initial={{ opacity: 0 }}
              animate={{ opacity: 1 }}
              exit={{ opacity: 0 }}
              className="absolute inset-0 grid place-items-center bg-white/60 text-neutral-700 backdrop-blur-[2px]"
            >
              {state === "off" ? (
                <div className="flex flex-col items-center gap-3 text-center">
                  <QrCode size={28} className="text-neutral-500" />
                  <p className="text-[0.8125rem] font-medium">Pairing code is off</p>
                  <Button variant="primary" size="sm" onClick={refresh} loading={refreshing}>
                    Show a code
                  </Button>
                </div>
              ) : (
                <div className="flex flex-col items-center gap-2 text-[0.8125rem] font-medium">
                  <Spinner size={22} />
                  {state === "preparing" ? "Preparing a code…" : "Refreshing the code…"}
                </div>
              )}
            </motion.div>
          )}
        </AnimatePresence>
      </div>

      <div className="mt-4 flex items-center gap-3">
        <div className="min-w-0 flex-1">
          <div className="flex items-center justify-between text-xs">
            <span className="flex items-center gap-1.5 text-fg-muted">
              <Clock3 size={12} />
              {state === "ready" ? "Code expires in" : state === "off" ? "No active code" : "Waiting"}
            </span>
            <span className={cn("font-mono tabular-nums transition-colors", urgent ? "text-warning" : "text-fg-muted")}>
              {state === "ready" ? formatCountdown(seconds) : "—"}
            </span>
          </div>
          <div className="mt-1.5 h-1 overflow-hidden rounded-full bg-surface-3">
            <motion.div
              className={cn("h-full rounded-full", urgent ? "bg-warning" : "bg-accent")}
              animate={{ width: `${state === "ready" ? progress * 100 : 0}%` }}
              transition={{ duration: 0.9, ease: "linear" }}
            />
          </div>
        </div>
        <Tooltip label="New code" side="top">
          <Button
            variant="secondary"
            size="md"
            aria-label="Generate a new code"
            onClick={refresh}
            disabled={refreshing || state === "preparing"}
            className="w-8 px-0"
          >
            <RefreshCw size={14} className={cn("transition-transform duration-500", refreshing && "animate-spin")} />
          </Button>
        </Tooltip>
      </div>

      <div className="mt-4 flex items-center justify-center gap-2 rounded-xl bg-surface-2 py-2 text-xs text-fg-muted">
        <StatusDot tone={snapshot?.listening ? "live" : "warn"} pulse />
        {snapshot?.listening ? "Listening for your phone on this network" : "Opening the listener…"}
      </div>
    </div>
  );
}

/** Viewfinder corners around the code; they breathe while the code is valid. */
function Corners({ active }: { active: boolean }) {
  const base = "pointer-events-none absolute size-6 border-accent transition-all duration-500";
  return (
    <div aria-hidden="true" className={cn("absolute inset-2 z-10", active ? "opacity-100" : "opacity-0")}>
      <span className={cn(base, "left-0 top-0 rounded-tl-lg border-l-[3px] border-t-[3px]")} />
      <span className={cn(base, "right-0 top-0 rounded-tr-lg border-r-[3px] border-t-[3px]")} />
      <span className={cn(base, "bottom-0 left-0 rounded-bl-lg border-b-[3px] border-l-[3px]")} />
      <span className={cn(base, "bottom-0 right-0 rounded-br-lg border-b-[3px] border-r-[3px]")} />
    </div>
  );
}
