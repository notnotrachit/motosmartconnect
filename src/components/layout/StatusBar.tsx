import { AnimatePresence, motion } from "motion/react";
import { ChevronUp, ClipboardCheck, ClipboardX, Wifi } from "lucide-react";
import { StatusDot } from "@/components/ui/Feedback";
import { Tooltip } from "@/components/ui/Tooltip";
import { parseLog } from "@/lib/format";
import { useEngine, useLog } from "@/store/engine";
import { useUi } from "@/store/ui";

/** Thin footer: listener state, address, clipboard sync and the latest activity line. */
export function StatusBar() {
  const listening = useEngine((s) => s.snapshot?.listening ?? false);
  const lanIp = useEngine((s) => s.snapshot?.lanIp ?? "");
  const port = useEngine((s) => s.snapshot?.port ?? 0);
  const clipboard = useEngine((s) => s.snapshot?.clipboardReady ?? false);
  const hasSnapshot = useEngine((s) => Boolean(s.snapshot));
  const log = useLog();
  const setActivity = useUi((s) => s.setActivity);
  const last = log.length ? parseLog(log[log.length - 1]) : null;

  return (
    <footer className="relative z-20 flex h-7 shrink-0 items-center gap-4 border-t border-line bg-sidebar/85 px-3 text-[11px] text-fg-subtle backdrop-blur-xl">
      <span className="flex items-center gap-1.5">
        <StatusDot tone={listening ? "live" : "warn"} pulse={!listening && hasSnapshot} />
        {listening ? "Listening" : hasSnapshot ? "Starting listener…" : "Starting…"}
      </span>

      {lanIp && (
        <Tooltip label="This Mac's address on the local network" side="top">
          <span className="flex items-center gap-1.5 font-mono tabular-nums">
            <Wifi size={11} />
            {lanIp}
            {port ? `:${port}` : ""}
          </span>
        </Tooltip>
      )}

      {hasSnapshot && (
        <Tooltip label={clipboard ? "Copied text syncs both ways" : "Turn on clipboard sync on the phone"} side="top">
          <span className="flex items-center gap-1.5">
            {clipboard ? <ClipboardCheck size={11} className="text-success" /> : <ClipboardX size={11} />}
            Clipboard {clipboard ? "on" : "off"}
          </span>
        </Tooltip>
      )}

      <button
        type="button"
        onClick={() => setActivity(true)}
        className="group ml-auto flex min-w-0 max-w-[50%] items-center gap-2 rounded px-1.5 py-0.5 transition hover:bg-surface-2 hover:text-fg-muted"
        aria-label="Open activity log"
      >
        <AnimatePresence mode="popLayout" initial={false}>
          {last && (
            <motion.span
              key={log.length + last.text}
              initial={{ opacity: 0, y: 6 }}
              animate={{ opacity: 1, y: 0 }}
              exit={{ opacity: 0, y: -6 }}
              transition={{ duration: 0.2 }}
              className="truncate"
            >
              {last.time && <span className="mr-1.5 font-mono opacity-70">{last.time}</span>}
              {last.text}
            </motion.span>
          )}
        </AnimatePresence>
        <ChevronUp size={11} className="shrink-0 opacity-60 transition group-hover:opacity-100" />
      </button>
    </footer>
  );
}
