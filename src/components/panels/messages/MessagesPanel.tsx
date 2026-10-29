import { useCallback } from "react";
import { AnimatePresence, motion } from "motion/react";
import { MessagesSquare, SquarePen } from "lucide-react";
import { Button } from "@/components/ui/Button";
import { Kbd } from "@/components/ui/Kbd";
import { closeThread, openThread } from "@/engine/actions";
import { cn } from "@/lib/cn";
import { useConnectedPhone, useMessages } from "@/store/engine";
import { Conversation } from "./Conversation";
import { NewMessage } from "./NewMessage";
import { ThreadList } from "./ThreadList";
import { useMessageUi } from "@/store/messages";

/**
 * Two panes when there is room (list + conversation), one pane otherwise.
 * The split follows the panel's own width through a container query, so it
 * adapts to the window and to the phone column beside it.
 */
export function MessagesPanel() {
  const sms = useMessages();
  const connected = Boolean(useConnectedPhone());
  const composing = useMessageUi((s) => s.composing);
  const stopCompose = useMessageUi((s) => s.stopCompose);
  const startCompose = useMessageUi((s) => s.startCompose);
  const setOpening = useMessageUi((s) => s.setOpening);
  const canSend = connected && sms.phase === "ready";
  const detail = composing ? "compose" : sms.openKey ? "thread" : "none";

  const open = useCallback(
    async (key: string) => {
      stopCompose();
      setOpening(key);
      await openThread(key);
      setOpening("");
    },
    [stopCompose, setOpening],
  );

  const close = useCallback(() => {
    if (useMessageUi.getState().composing) stopCompose();
    else void closeThread();
  }, [stopCompose]);

  return (
    <div className="@container flex min-h-0 flex-1">
      <ThreadList
        sms={sms}
        connected={connected}
        canSend={canSend}
        onOpen={(key) => void open(key)}
        className={cn(
          "w-full @2xl:w-[300px] @2xl:shrink-0 @2xl:border-r @2xl:border-line @4xl:w-[340px]",
          detail !== "none" && "hidden @2xl:flex",
        )}
      />

      <section
        aria-label="Conversation"
        className={cn("relative min-w-0 flex-1 flex-col bg-canvas", detail === "none" ? "hidden @2xl:flex" : "flex")}
      >
        <AnimatePresence mode="wait" initial={false}>
          <motion.div
            key={detail === "thread" ? `t-${sms.openKey}` : detail}
            initial={{ opacity: 0, x: 10 }}
            animate={{ opacity: 1, x: 0 }}
            exit={{ opacity: 0, x: -6 }}
            transition={{ duration: 0.16, ease: [0.16, 1, 0.3, 1] }}
            className="flex min-h-0 flex-1 flex-col"
          >
            {detail === "compose" ? (
              <NewMessage threads={sms.threads} canSend={canSend} onOpenThread={(key) => void open(key)} />
            ) : detail === "thread" ? (
              <Conversation sms={sms} connected={connected} canSend={canSend} onClose={close} />
            ) : (
              <NothingOpen canSend={canSend} onCompose={startCompose} hasThreads={sms.threads.length > 0} />
            )}
          </motion.div>
        </AnimatePresence>
      </section>
    </div>
  );
}

function NothingOpen({ canSend, onCompose, hasThreads }: { canSend: boolean; onCompose: () => void; hasThreads: boolean }) {
  return (
    <div className="flex flex-1 flex-col items-center justify-center px-8 text-center">
      <div className="relative mb-5">
        <div className="absolute inset-0 -z-10 scale-150 rounded-full bg-accent/10 blur-2xl" />
        <span className="grid size-14 place-items-center rounded-2xl bg-surface ring-1 ring-line shadow-card">
          <MessagesSquare size={24} className="text-accent" />
        </span>
      </div>
      <p className="text-sm font-semibold text-fg">{hasThreads ? "Pick a conversation" : "Your texts, on your Mac"}</p>
      <p className="mt-1.5 max-w-[30ch] text-xs leading-relaxed text-fg-muted">
        {hasThreads ? "Read and reply from here. Texts are sent through your phone." : "Send and receive SMS through your phone."}
      </p>
      {canSend && (
        <Button variant="primary" className="mt-5" icon={<SquarePen size={13} />} onClick={onCompose}>
          New message <Kbd className="ml-1 border-white/25 bg-white/15 text-on-accent">⌘N</Kbd>
        </Button>
      )}
    </div>
  );
}
