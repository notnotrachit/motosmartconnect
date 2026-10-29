import { useState, type ReactNode } from "react";
import { AnimatePresence, motion } from "motion/react";
import { AlertTriangle, Camera, Monitor, ScreenShare, WifiOff } from "lucide-react";
import { Button } from "@/components/ui/Button";
import { startDesktop, startMirror, startWebcam, stopStream } from "@/engine/actions";
import type { ScreenPhase } from "./screenPhase";

interface ScreenOverlayProps {
  phase: ScreenPhase;
  label: string;
}

/** Everything drawn over the phone screen before a picture arrives. */
export function ScreenOverlay({ phase, label }: ScreenOverlayProps) {
  return (
    <AnimatePresence mode="wait" initial={false}>
      {phase !== "live" && (
        <motion.div
          key={phase}
          initial={{ opacity: 0, scale: 0.98 }}
          animate={{ opacity: 1, scale: 1 }}
          exit={{ opacity: 0, scale: 0.98, transition: { duration: 0.12 } }}
          transition={{ duration: 0.25, ease: [0.16, 1, 0.3, 1] }}
          className="pointer-events-none absolute inset-0 z-[3] flex flex-col items-center justify-center px-6 text-center text-white"
        >
          {phase === "idle" && <Idle />}
          {phase === "offline" && (
            <Message icon={<WifiOff size={22} />} title="Phone is offline">
              Open Smart Connect on the phone to reconnect. Both devices need the same Wi-Fi.
            </Message>
          )}
          {phase === "busy" && <Busy label={label} />}
          {phase === "error" && <Failed label={label} />}
        </motion.div>
      )}
    </AnimatePresence>
  );
}

function Message({ icon, title, children, tone = "default" }: { icon: ReactNode; title: string; children: ReactNode; tone?: "default" | "error" }) {
  return (
    <>
      <div
        className={
          tone === "error"
            ? "mb-4 grid size-12 place-items-center rounded-2xl bg-red-500/15 text-red-300 ring-1 ring-red-400/25"
            : "mb-4 grid size-12 place-items-center rounded-2xl bg-white/[0.06] text-white/70 ring-1 ring-white/10"
        }
      >
        {icon}
      </div>
      <p className="text-sm font-semibold">{title}</p>
      <p className="mt-1.5 max-w-[26ch] text-xs leading-relaxed text-white/55">{children}</p>
    </>
  );
}

function Idle() {
  const [busy, setBusy] = useState(false);
  const mirror = async () => {
    setBusy(true);
    await startMirror();
    setBusy(false);
  };
  return (
    <>
      <div className="relative mb-5 grid size-14 place-items-center">
        <span className="absolute inset-0 animate-ping-slow rounded-2xl bg-accent/25" />
        <span className="relative grid size-14 place-items-center rounded-2xl bg-accent/20 text-accent ring-1 ring-accent/30">
          <ScreenShare size={24} />
        </span>
      </div>
      <p className="text-[0.95rem] font-semibold">Mirror your phone</p>
      <p className="mt-1.5 max-w-[24ch] text-xs leading-relaxed text-white/55">
        See the screen here, then click and type to control it.
      </p>
      <Button variant="primary" size="lg" className="pointer-events-auto mt-5" loading={busy} onClick={mirror}>
        Start mirroring
      </Button>
      <div className="pointer-events-auto mt-3 flex gap-1">
        <button
          type="button"
          onClick={() => void startDesktop()}
          className="flex items-center gap-1.5 rounded-lg px-2.5 py-1.5 text-xs text-white/60 transition hover:bg-white/10 hover:text-white"
        >
          <Monitor size={13} /> Desktop
        </button>
        <button
          type="button"
          onClick={() => void startWebcam()}
          className="flex items-center gap-1.5 rounded-lg px-2.5 py-1.5 text-xs text-white/60 transition hover:bg-white/10 hover:text-white"
        >
          <Camera size={13} /> Camera
        </button>
      </div>
    </>
  );
}

function Busy({ label }: { label: string }) {
  return (
    <>
      <div className="relative mb-5 size-12">
        <svg viewBox="0 0 48 48" className="size-12 animate-spin-fast text-accent">
          <circle cx="24" cy="24" r="20" fill="none" stroke="currentColor" strokeOpacity="0.15" strokeWidth="3" />
          <path d="M44 24a20 20 0 0 0-20-20" fill="none" stroke="currentColor" strokeWidth="3" strokeLinecap="round" />
        </svg>
      </div>
      <p className="text-sm font-semibold">Connecting to the screen</p>
      <AnimatePresence mode="wait" initial={false}>
        <motion.p
          key={label}
          initial={{ opacity: 0, y: 4 }}
          animate={{ opacity: 1, y: 0 }}
          exit={{ opacity: 0, y: -4 }}
          transition={{ duration: 0.18 }}
          className="mt-2 line-clamp-4 max-w-[30ch] text-xs leading-relaxed text-white/55"
          aria-live="polite"
        >
          {label}
        </motion.p>
      </AnimatePresence>
      <button
        type="button"
        onClick={() => void stopStream()}
        className="pointer-events-auto mt-5 rounded-lg px-3 py-1.5 text-xs text-white/60 transition hover:bg-white/10 hover:text-white"
      >
        Cancel
      </button>
    </>
  );
}

function Failed({ label }: { label: string }) {
  const [busy, setBusy] = useState(false);
  const retry = async () => {
    setBusy(true);
    await startMirror();
    setBusy(false);
  };
  return (
    <>
      <Message icon={<AlertTriangle size={22} />} title="Couldn't show the screen" tone="error">
        {label}
      </Message>
      <Button variant="primary" className="pointer-events-auto mt-5" loading={busy} onClick={retry}>
        Try again
      </Button>
    </>
  );
}
