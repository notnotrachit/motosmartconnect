import { AnimatePresence, motion } from "motion/react";
import { AppWindow } from "lucide-react";
import { showSession } from "@/engine/actions";
import { cn } from "@/lib/cn";
import type { SessionView } from "@/lib/types";

/** Open screen sessions. The highlighted one is on the phone well. */
export function SessionChips({ sessions, active }: { sessions: SessionView[]; active: string }) {
  return (
    <div className="flex min-h-0 flex-wrap justify-center gap-1.5">
      <AnimatePresence initial={false}>
        {sessions.map((session) => {
          const on = session.sessionId === active;
          return (
            <motion.button
              layout
              key={session.sessionId}
              type="button"
              initial={{ opacity: 0, scale: 0.9 }}
              animate={{ opacity: 1, scale: 1 }}
              exit={{ opacity: 0, scale: 0.9 }}
              onClick={() => showSession(session.sessionId, session.pkg, session.title)}
              aria-pressed={on}
              className={cn(
                "flex h-6 max-w-[180px] items-center gap-1.5 rounded-full border px-2.5 text-[11px] font-medium transition",
                on
                  ? "border-accent/40 bg-accent/15 text-accent"
                  : "border-line bg-surface text-fg-muted hover:border-line-strong hover:text-fg",
              )}
            >
              {on ? <span className="size-1.5 rounded-full bg-accent" /> : <AppWindow size={11} />}
              <span className="truncate">{session.title || "Screen"}</span>
            </motion.button>
          );
        })}
      </AnimatePresence>
    </div>
  );
}
