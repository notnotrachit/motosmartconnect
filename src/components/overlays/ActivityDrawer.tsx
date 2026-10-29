import { useDeferredValue, useEffect, useLayoutEffect, useMemo, useRef, useState } from "react";
import { AnimatePresence, motion } from "motion/react";
import { ArrowDown, Search, X } from "lucide-react";
import { IconButton } from "@/components/ui/IconButton";
import { Input } from "@/components/ui/Input";
import { useFocusTrap } from "@/hooks/useFocusTrap";
import { useLayer } from "@/hooks/useLayer";
import { cn } from "@/lib/cn";
import { parseLog } from "@/lib/format";
import { useLog } from "@/store/engine";
import { useUi } from "@/store/ui";
import { CopyButton } from "@/components/ui/CopyButton";

const PROBLEM = /fail|error|could not|did not|rejected|declined|refused/i;

/** Slide-over with the engine's activity log: filterable, copyable, auto-following. */
export function ActivityDrawer() {
  const open = useUi((s) => s.activityOpen);
  const setActivity = useUi((s) => s.setActivity);
  const close = () => setActivity(false);
  const panel = useRef<HTMLDivElement>(null);
  useLayer(open, close);
  useFocusTrap(panel, open);

  return (
    <AnimatePresence>
      {open && (
        <div className="fixed inset-x-0 bottom-7 top-[52px] z-40">
          <motion.div
            className="absolute inset-0 bg-black/15 dark:bg-black/35"
            initial={{ opacity: 0 }}
            animate={{ opacity: 1 }}
            exit={{ opacity: 0 }}
            onClick={close}
          />
          <motion.aside
            ref={panel}
            role="dialog"
            aria-label="Activity"
            initial={{ x: "100%" }}
            animate={{ x: 0 }}
            exit={{ x: "100%", transition: { duration: 0.2, ease: [0.4, 0, 1, 1] } }}
            transition={{ type: "spring", duration: 0.45, bounce: 0.08 }}
            className="absolute bottom-0 right-0 top-0 flex w-[min(460px,90vw)] flex-col border-l border-line bg-surface shadow-pop"
          >
            <LogView onClose={close} />
          </motion.aside>
        </div>
      )}
    </AnimatePresence>
  );
}

function LogView({ onClose }: { onClose: () => void }) {
  const log = useLog();
  const [filter, setFilter] = useState("");
  const [follow, setFollow] = useState(true);
  const deferred = useDeferredValue(filter);
  const scroller = useRef<HTMLDivElement>(null);

  const lines = useMemo(() => {
    const parsed = log.map(parseLog);
    const q = deferred.trim().toLowerCase();
    return q ? parsed.filter((l) => l.text.toLowerCase().includes(q)) : parsed;
  }, [log, deferred]);

  useLayoutEffect(() => {
    if (follow && scroller.current) scroller.current.scrollTop = scroller.current.scrollHeight;
  }, [lines, follow]);

  useEffect(() => {
    const el = scroller.current;
    if (!el) return;
    const onScroll = () => setFollow(el.scrollHeight - el.scrollTop - el.clientHeight < 24);
    el.addEventListener("scroll", onScroll, { passive: true });
    return () => el.removeEventListener("scroll", onScroll);
  }, []);

  return (
    <>
      <header className="flex items-center gap-2 border-b border-line px-4 py-3">
        <div className="min-w-0 flex-1">
          <h2 className="font-display text-[0.95rem] font-semibold tracking-tight text-fg">Activity</h2>
          <p className="text-[11px] text-fg-subtle">{log.length} events this session</p>
        </div>
        <CopyButton value={log.join("\n")} label="Copy log" />
        <IconButton label="Close" shortcut="esc" icon={<X size={16} />} size="sm" onClick={onClose} />
      </header>
      <div className="border-b border-line px-4 py-2.5">
        <Input
          data-autofocus
          value={filter}
          onChange={(event) => setFilter(event.target.value)}
          onClear={() => setFilter("")}
          placeholder="Filter events"
          aria-label="Filter events"
          leading={<Search size={13} />}
        />
      </div>

      <div ref={scroller} className="relative min-h-0 flex-1 overflow-y-auto py-2 font-mono text-[11.5px] leading-relaxed" aria-live="polite">
        {!lines.length ? (
          <p className="px-4 py-10 text-center font-sans text-[0.8125rem] text-fg-muted">{log.length ? "No matching events" : "Nothing yet"}</p>
        ) : (
          <ol>
            {lines.map((line, i) => (
              <li
                key={`${i}-${line.time}`}
                className={cn("flex select-text gap-3 px-4 py-0.5 hover:bg-surface-2", PROBLEM.test(line.text) ? "text-danger" : "text-fg-muted")}
              >
                <span className="shrink-0 tabular-nums text-fg-subtle">{line.time || "—"}</span>
                <span className="min-w-0 [overflow-wrap:anywhere]">{line.text}</span>
              </li>
            ))}
          </ol>
        )}
      </div>

      <AnimatePresence>
        {!follow && (
          <motion.button
            type="button"
            initial={{ opacity: 0, y: 8 }}
            animate={{ opacity: 1, y: 0 }}
            exit={{ opacity: 0, y: 8 }}
            onClick={() => setFollow(true)}
            className="absolute bottom-4 left-1/2 flex -translate-x-1/2 items-center gap-1.5 rounded-full bg-fg px-3 py-1.5 text-xs font-medium text-canvas shadow-pop"
          >
            <ArrowDown size={12} /> Latest
          </motion.button>
        )}
      </AnimatePresence>
    </>
  );
}
