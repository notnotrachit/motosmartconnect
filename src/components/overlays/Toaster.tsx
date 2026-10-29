import { useEffect, useRef, useState } from "react";
import { AnimatePresence, motion, useAnimationControls } from "motion/react";
import { AlertCircle, AlertTriangle, CheckCircle2, Info, X } from "lucide-react";
import { cn } from "@/lib/cn";
import { useToasts, type Toast, type ToastTone } from "@/store/toast";

const TONES: Record<ToastTone, { icon: typeof Info; color: string; bar: string }> = {
  info: { icon: Info, color: "text-accent", bar: "bg-accent" },
  success: { icon: CheckCircle2, color: "text-success", bar: "bg-success" },
  warning: { icon: AlertTriangle, color: "text-warning", bar: "bg-warning" },
  error: { icon: AlertCircle, color: "text-danger", bar: "bg-danger" },
};

/** Stack of toasts in the bottom-right corner, above the status bar. */
export function Toaster() {
  const toasts = useToasts((s) => s.toasts);
  return (
    <div
      aria-live="polite"
      aria-relevant="additions"
      className="pointer-events-none fixed bottom-10 right-4 z-[70] flex w-[min(360px,calc(100vw-2rem))] flex-col-reverse gap-2"
    >
      <AnimatePresence initial={false}>
        {[...toasts].reverse().map((t) => (
          <ToastItem key={t.id} toast={t} />
        ))}
      </AnimatePresence>
    </div>
  );
}

function ToastItem({ toast }: { toast: Toast }) {
  const dismiss = useToasts((s) => s.dismiss);
  const [paused, setPaused] = useState(false);
  const remaining = useRef(toast.duration);
  const startedAt = useRef(0);
  const shake = useAnimationControls();
  const tone = TONES[toast.tone];
  const Icon = tone.icon;

  // A repeat of the same toast restarts the timer and nudges the card.
  useEffect(() => {
    remaining.current = toast.duration;
    if (toast.repeat > 0) void shake.start({ x: [0, -6, 6, -3, 3, 0], transition: { duration: 0.35 } });
  }, [toast.repeat, toast.duration, shake]);

  useEffect(() => {
    if (paused) return;
    startedAt.current = Date.now();
    const id = window.setTimeout(() => dismiss(toast.id), remaining.current);
    return () => {
      window.clearTimeout(id);
      remaining.current = Math.max(600, remaining.current - (Date.now() - startedAt.current));
    };
  }, [paused, toast.id, toast.repeat, dismiss]);

  return (
    <motion.div
      layout
      initial={{ opacity: 0, y: 16, scale: 0.96 }}
      animate={{ opacity: 1, y: 0, scale: 1 }}
      exit={{ opacity: 0, x: 40, scale: 0.98, transition: { duration: 0.18 } }}
      transition={{ type: "spring", duration: 0.42, bounce: 0.2 }}
      className="pointer-events-auto"
      onMouseEnter={() => setPaused(true)}
      onMouseLeave={() => setPaused(false)}
    >
      <motion.div
        animate={shake}
        role={toast.tone === "error" ? "alert" : "status"}
        className="relative overflow-hidden rounded-xl border border-line bg-glass shadow-pop backdrop-blur-xl"
      >
        <div className="flex items-start gap-3 py-3 pl-3.5 pr-2.5">
          <Icon size={18} className={cn("mt-px shrink-0", tone.color)} />
          <div className="min-w-0 flex-1">
            <p className="text-[0.8125rem] font-medium leading-snug text-fg [overflow-wrap:anywhere]">{toast.title}</p>
            {toast.body && <p className="mt-0.5 text-xs leading-relaxed text-fg-muted [overflow-wrap:anywhere]">{toast.body}</p>}
            {toast.action && (
              <button
                type="button"
                onClick={() => {
                  toast.action?.run();
                  dismiss(toast.id);
                }}
                className="mt-2 rounded-md text-xs font-semibold text-accent transition hover:text-accent-strong"
              >
                {toast.action.label}
              </button>
            )}
          </div>
          <button
            type="button"
            aria-label="Dismiss"
            onClick={() => dismiss(toast.id)}
            className="grid size-6 shrink-0 place-items-center rounded-md text-fg-subtle transition hover:bg-surface-3 hover:text-fg"
          >
            <X size={13} />
          </button>
        </div>
        <span
          key={toast.repeat}
          aria-hidden="true"
          className={cn("absolute bottom-0 left-0 h-[2px] w-full origin-left opacity-60", tone.bar)}
          style={{
            animation: `toast-progress ${toast.duration}ms linear forwards`,
            animationPlayState: paused ? "paused" : "running",
          }}
        />
      </motion.div>
    </motion.div>
  );
}
