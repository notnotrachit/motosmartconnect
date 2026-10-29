import { useCallback, useEffect, useLayoutEffect, useMemo, useRef, useState } from "react";
import { AnimatePresence, motion } from "motion/react";
import { ArrowDown, ChevronLeft, X } from "lucide-react";
import { Avatar } from "@/components/ui/Avatar";
import { CopyButton } from "@/components/ui/CopyButton";
import { IconButton } from "@/components/ui/IconButton";
import { Skeleton } from "@/components/ui/Feedback";
import { sendThread } from "@/engine/actions";
import { useNow } from "@/hooks/useNow";
import type { SmsView } from "@/lib/types";
import { Composer } from "./Composer";
import { buildRows, isBareNumber } from "./format";
import { MessageBubble } from "./MessageBubble";
import { useMessageUi } from "@/store/messages";

interface ConversationProps {
  sms: SmsView;
  connected: boolean;
  canSend: boolean;
  onClose: () => void;
}

const NEAR_BOTTOM = 80;

export function Conversation({ sms, connected, canSend, onClose }: ConversationProps) {
  const now = useNow(60_000);
  const scroller = useRef<HTMLDivElement>(null);
  const draft = useMessageUi((s) => s.drafts[sms.openKey] ?? "");
  const setDraft = useMessageUi((s) => s.setDraft);
  const focusComposer = useMessageUi((s) => s.focusComposer);
  const [atBottom, setAtBottom] = useState(true);
  const [unseen, setUnseen] = useState(0);

  const rows = useMemo(() => buildRows(sms.open, now), [sms.open, now]);
  const title = sms.openTitle || sms.openAddress || "Conversation";
  const showAddress = sms.openAddress && sms.openAddress !== title;

  // Messages present when the thread opened render without an entrance animation.
  const seen = useRef<{ key: string; ids: Set<string> }>({ key: "", ids: new Set() });
  if (seen.current.key !== sms.openKey) seen.current = { key: sms.openKey, ids: new Set(sms.open.map((m) => m.id)) };
  const isFresh = (id: string) => !seen.current.ids.has(id);
  useEffect(() => {
    for (const m of sms.open) seen.current.ids.add(m.id);
  });

  const scrollToEnd = useCallback((smooth: boolean) => {
    const el = scroller.current;
    if (!el) return;
    el.scrollTo({ top: el.scrollHeight, behavior: smooth ? "smooth" : "auto" });
    setUnseen(0);
  }, []);

  // Jump to the newest message whenever another conversation opens.
  useLayoutEffect(() => {
    scrollToEnd(false);
  }, [sms.openKey, scrollToEnd]);

  // Follow new messages if already at the bottom or if this Mac sent them; otherwise count them.
  const last = sms.open[sms.open.length - 1];
  const lastId = last?.id;
  const prevLast = useRef(lastId);
  useEffect(() => {
    if (!lastId || lastId === prevLast.current) return;
    prevLast.current = lastId;
    if (atBottom || last?.outgoing) scrollToEnd(true);
    else setUnseen((n) => n + 1);
  }, [lastId, last?.outgoing, atBottom, scrollToEnd]);

  const onScroll = () => {
    const el = scroller.current;
    if (!el) return;
    const near = el.scrollHeight - el.scrollTop - el.clientHeight < NEAR_BOTTOM;
    setAtBottom(near);
    if (near) setUnseen(0);
  };

  const retry = (body: string) => {
    setDraft(sms.openKey, body);
    focusComposer();
  };

  const loading = !sms.open.length;

  return (
    <div className="flex min-h-0 flex-1 flex-col" onKeyDown={(event) => event.key === "Escape" && !event.defaultPrevented && onClose()}>
      <header className="flex h-[60px] shrink-0 items-center gap-3 border-b border-line px-4">
        <span className="@2xl:hidden">
          <IconButton label="Back to conversations" icon={<ChevronLeft size={16} />} size="sm" onClick={onClose} />
        </span>
        <Avatar name={title} anonymous={isBareNumber(title)} size={34} />
        <div className="min-w-0 flex-1">
          <h2 className="truncate text-[0.8125rem] font-semibold text-fg">{title}</h2>
          <p className="flex items-center gap-1 truncate text-[11px] text-fg-muted">
            {showAddress ? <span className="select-text font-mono">{sms.openAddress}</span> : "SMS via your phone"}
          </p>
        </div>
        {sms.openAddress && <CopyButton value={sms.openAddress} label="Copy number" />}
        <span className="hidden @2xl:inline-flex">
          <IconButton label="Close conversation" shortcut="esc" icon={<X size={15} />} size="sm" onClick={onClose} />
        </span>
      </header>

      <div className="relative min-h-0 flex-1">
        <div ref={scroller} onScroll={onScroll} className="h-full overflow-y-auto px-5 pb-4 pt-2">
          {loading ? (
            <ConversationSkeleton />
          ) : (
            <ol aria-label={`Messages with ${title}`} className="flex flex-col">
              {rows.map((row) =>
                row.kind === "day" ? (
                  <li key={row.key} className="sticky top-0 z-[1] my-3 flex justify-center">
                    <span className="rounded-full border border-line bg-canvas/85 px-2.5 py-0.5 text-[10.5px] font-medium text-fg-muted backdrop-blur">
                      {row.label}
                    </span>
                  </li>
                ) : (
                  <MessageBubble
                    key={row.key}
                    message={row.message!}
                    position={row.position!}
                    stamp={Boolean(row.stamp)}
                    fresh={isFresh(row.key)}
                    onRetry={retry}
                  />
                ),
              )}
            </ol>
          )}
        </div>

        <AnimatePresence>
          {!atBottom && !loading && (
            <motion.button
              type="button"
              initial={{ opacity: 0, y: 8, scale: 0.9 }}
              animate={{ opacity: 1, y: 0, scale: 1 }}
              exit={{ opacity: 0, y: 8, scale: 0.9 }}
              onClick={() => scrollToEnd(true)}
              className="absolute bottom-3 left-1/2 flex -translate-x-1/2 items-center gap-1.5 rounded-full border border-line bg-glass px-3 py-1.5 text-xs font-medium text-fg shadow-pop backdrop-blur-xl transition hover:bg-surface"
            >
              <ArrowDown size={12} />
              {unseen ? `${unseen} new message${unseen === 1 ? "" : "s"}` : "Latest"}
            </motion.button>
          )}
        </AnimatePresence>
      </div>

      <Composer
        value={draft}
        onChange={(text) => setDraft(sms.openKey, text)}
        onSend={sendThread}
        disabled={!canSend}
        placeholder={connected ? `Text ${title}` : "The phone is offline"}
        autoFocus
        onEscape={draft ? undefined : onClose}
      />
    </div>
  );
}

function ConversationSkeleton() {
  const widths = ["w-2/5", "w-3/5", "w-1/3", "w-1/2", "w-2/5"];
  return (
    <div aria-label="Loading messages" className="flex flex-col gap-3 py-4">
      {widths.map((w, i) => (
        <div key={i} className={i % 2 ? "flex justify-end" : "flex justify-start"}>
          <Skeleton className={`h-9 rounded-[18px] ${w}`} />
        </div>
      ))}
    </div>
  );
}
