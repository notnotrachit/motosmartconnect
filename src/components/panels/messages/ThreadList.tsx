import { useDeferredValue, useMemo, useState, type KeyboardEvent } from "react";
import { AnimatePresence } from "motion/react";
import { Search, SearchX, SquarePen, WifiOff } from "lucide-react";
import { Button } from "@/components/ui/Button";
import { IconButton } from "@/components/ui/IconButton";
import { Input } from "@/components/ui/Input";
import { useNow } from "@/hooks/useNow";
import { cn } from "@/lib/cn";
import { fuzzyScore } from "@/lib/format";
import type { SmsView } from "@/lib/types";
import { MessagesStatus, type MessagesPhase } from "./MessagesStatus";
import { ThreadRow } from "./ThreadRow";
import { useMessageUi } from "@/store/messages";

interface ThreadListProps {
  sms: SmsView;
  connected: boolean;
  canSend: boolean;
  onOpen: (key: string) => void;
  className?: string;
}

/** Conversation list: search, new-message, unread markers and arrow-key navigation. */
export function ThreadList({ sms, connected, canSend, onOpen, className }: ThreadListProps) {
  const [query, setQuery] = useState("");
  const deferred = useDeferredValue(query);
  const now = useNow(30_000);
  const drafts = useMessageUi((s) => s.drafts);
  const opening = useMessageUi((s) => s.opening);
  const composing = useMessageUi((s) => s.composing);
  const startCompose = useMessageUi((s) => s.startCompose);
  const phase = sms.phase as MessagesPhase;

  const shown = useMemo(() => {
    const q = deferred.trim();
    if (!q) return sms.threads;
    return sms.threads
      .map((thread) => ({ thread, score: fuzzyScore(q, `${thread.title} ${thread.address} ${thread.preview}`) }))
      .filter((item) => item.score >= 0)
      .sort((a, b) => b.score - a.score)
      .map((item) => item.thread);
  }, [sms.threads, deferred]);

  const unread = sms.threads.reduce((sum, thread) => sum + (thread.unread > 0 ? 1 : 0), 0);

  // ↑/↓ moves between conversations; Enter/Space opens (native button behaviour).
  const onKeyDown = (event: KeyboardEvent<HTMLUListElement>) => {
    if (event.key !== "ArrowDown" && event.key !== "ArrowUp") return;
    const rows = [...event.currentTarget.querySelectorAll<HTMLButtonElement>("[data-thread]")];
    const at = rows.indexOf(document.activeElement as HTMLButtonElement);
    const next = rows[event.key === "ArrowDown" ? Math.min(rows.length - 1, at + 1) : Math.max(0, at - 1)];
    if (!next) return;
    event.preventDefault();
    next.focus();
  };

  return (
    <div className={cn("flex min-h-0 flex-col", className)}>
      <div className="flex shrink-0 items-center gap-2 px-4 pb-3 pt-5">
        <div className="min-w-0 flex-1">
          <h2 className="font-display text-[1.05rem] font-semibold tracking-tight text-fg">Messages</h2>
          <p className="mt-0.5 truncate text-xs text-fg-muted">
            {phase === "ready"
              ? unread
                ? `${unread} unread · ${sms.threads.length} conversations`
                : `${sms.threads.length} conversation${sms.threads.length === 1 ? "" : "s"}`
              : phase === "loading"
                ? "Loading recent texts…"
                : "Texts from your phone"}
          </p>
        </div>
        <IconButton
          label="New message"
          shortcut="⌘N"
          icon={<SquarePen size={15} />}
          variant="solid"
          active={composing}
          disabled={!canSend}
          onClick={startCompose}
        />
      </div>

      <div className="shrink-0 px-4 pb-2">
        <Input
          value={query}
          onChange={(event) => setQuery(event.target.value)}
          onClear={() => setQuery("")}
          onKeyDown={(event) => {
            if (event.key === "Enter" && shown[0]) onOpen(shown[0].key);
            if (event.key === "ArrowDown") {
              event.preventDefault();
              document.querySelector<HTMLButtonElement>("[data-thread]")?.focus();
            }
          }}
          placeholder="Search messages"
          aria-label="Search messages"
          leading={<Search size={13} />}
          disabled={!sms.threads.length}
        />
      </div>

      {!connected && sms.threads.length > 0 && (
        <div className="mx-4 mb-2 flex items-center gap-2 rounded-lg border border-warning/25 bg-warning/10 px-2.5 py-1.5 text-[11px] text-warning">
          <WifiOff size={12} />
          Phone offline. Showing the last texts it sent.
        </div>
      )}

      <div className="min-h-0 flex-1 overflow-y-auto px-1.5 pb-4">
        {!sms.threads.length ? (
          phase === "ready" ? (
            <div className="px-6 py-12 text-center">
              <p className="text-[0.8125rem] font-semibold text-fg">No recent texts</p>
              <p className="mt-1 text-xs text-fg-muted">The phone shares about the last 10 days.</p>
              {canSend && (
                <Button size="sm" variant="primary" className="mt-4" icon={<SquarePen size={12} />} onClick={startCompose}>
                  New message
                </Button>
              )}
            </div>
          ) : (
            <MessagesStatus phase={phase} connected={connected} />
          )
        ) : !shown.length ? (
          <div className="flex flex-col items-center px-6 py-12 text-center">
            <SearchX size={20} className="mb-2 text-fg-subtle" />
            <p className="text-[0.8125rem] font-medium text-fg">No matches</p>
            <p className="mt-1 text-xs text-fg-muted">Nothing matches “{query}”.</p>
            <Button size="sm" className="mt-3" onClick={() => setQuery("")}>
              Clear search
            </Button>
          </div>
        ) : (
          <ul aria-label="Conversations" onKeyDown={onKeyDown} className="flex flex-col gap-px">
            <AnimatePresence initial={false}>
              {shown.map((thread) => (
                <ThreadRow
                  key={thread.key}
                  thread={thread}
                  selected={thread.key === sms.openKey && !composing}
                  opening={thread.key === opening}
                  draft={drafts[thread.key] ?? ""}
                  query={deferred}
                  now={now}
                  onOpen={onOpen}
                />
              ))}
            </AnimatePresence>
          </ul>
        )}
      </div>
    </div>
  );
}
