import { memo, useState } from "react";
import { motion } from "motion/react";
import { AlertCircle, Check, Clock3, Copy, KeyRound, RotateCcw } from "lucide-react";
import { cn } from "@/lib/cn";
import type { MessageView } from "@/lib/types";
import { toast } from "@/store/toast";
import { clock, oneTimeCode, type BubblePosition } from "./format";

interface MessageBubbleProps {
  message: MessageView;
  position: BubblePosition;
  stamp: boolean;
  /** Animate in (only for messages that arrived while the thread was open). */
  fresh: boolean;
  onRetry: (body: string) => void;
}

// Corners tighten on the sender's side inside a run, like iMessage.
const OUT: Record<BubblePosition, string> = {
  single: "rounded-[18px] rounded-br-[6px]",
  first: "rounded-[18px] rounded-br-[6px]",
  middle: "rounded-[18px] rounded-r-[6px]",
  last: "rounded-[18px] rounded-tr-[6px]",
};
const IN: Record<BubblePosition, string> = {
  single: "rounded-[18px] rounded-bl-[6px]",
  first: "rounded-[18px] rounded-bl-[6px]",
  middle: "rounded-[18px] rounded-l-[6px]",
  last: "rounded-[18px] rounded-tl-[6px]",
};

async function copy(text: string, what: string) {
  try {
    await navigator.clipboard.writeText(text);
    toast.success(`${what} copied`);
  } catch {
    toast.error("Couldn't copy to the clipboard");
  }
}

export const MessageBubble = memo(function MessageBubble({ message, position, stamp, fresh, onRetry }: MessageBubbleProps) {
  const out = message.outgoing;
  const code = out ? "" : oneTimeCode(message.body);
  const [codeCopied, setCodeCopied] = useState(false);
  const joinedAbove = position === "middle" || position === "last";

  return (
    <motion.li
      layout="position"
      initial={fresh ? { opacity: 0, y: 12, scale: 0.96 } : false}
      animate={{ opacity: 1, y: 0, scale: 1 }}
      transition={{ type: "spring", duration: 0.35, bounce: 0.18 }}
      style={{ transformOrigin: out ? "bottom right" : "bottom left" }}
      className={cn("group flex flex-col", out ? "items-end" : "items-start", joinedAbove ? "mt-0.5" : "mt-2.5")}
    >
      <div className={cn("flex max-w-[78%] items-center gap-1.5", out ? "flex-row-reverse" : "flex-row")}>
        <div
          title={message.date ? new Date(message.date).toLocaleString() : undefined}
          className={cn(
            "min-w-0 px-3 py-[7px] text-[0.8125rem] leading-[1.45] transition-opacity",
            out ? OUT[position] : IN[position],
            out
              ? message.failed
                ? "bg-danger/12 text-fg ring-1 ring-danger/40"
                : "bg-accent text-on-accent"
              : "bg-surface-2 text-fg ring-1 ring-line dark:bg-surface-3",
            message.pending && "opacity-65",
          )}
        >
          <p className="select-text whitespace-pre-wrap [overflow-wrap:anywhere]">{message.body || <em className="opacity-70">Attachment</em>}</p>
        </div>

        {/* Hover tools beside the bubble. */}
        <button
          type="button"
          aria-label="Copy message"
          onClick={() => void copy(message.body, "Message")}
          className="grid size-6 shrink-0 place-items-center rounded-full text-fg-subtle opacity-0 transition hover:bg-surface-2 hover:text-fg focus-visible:opacity-100 group-hover:opacity-100"
        >
          <Copy size={12} />
        </button>
      </div>

      {code && (
        <button
          type="button"
          onClick={() => {
            void copy(code, "Code");
            setCodeCopied(true);
            window.setTimeout(() => setCodeCopied(false), 1600);
          }}
          className="mt-1 flex items-center gap-1.5 rounded-full border border-accent/30 bg-accent/10 px-2.5 py-1 text-[11px] font-medium text-accent transition hover:bg-accent/15 active:scale-95"
        >
          {codeCopied ? <Check size={11} /> : <KeyRound size={11} />}
          {codeCopied ? "Copied" : <>Copy code <span className="font-mono tracking-wider">{code}</span></>}
        </button>
      )}

      {stamp && (
        <span className={cn("mt-1 flex items-center gap-1 px-1 text-[10.5px]", message.failed ? "text-danger" : "text-fg-subtle")}>
          {message.failed ? (
            <>
              <AlertCircle size={11} /> Not sent
              <button
                type="button"
                onClick={() => onRetry(message.body)}
                className="ml-1 inline-flex items-center gap-1 rounded font-semibold text-danger underline-offset-2 hover:underline"
              >
                <RotateCcw size={10} /> Edit & resend
              </button>
            </>
          ) : message.pending ? (
            <>
              <Clock3 size={10} className="animate-pulse" /> Sending…
            </>
          ) : message.date ? (
            <>
              {clock(message.date)}
              {out && <Check size={11} className="text-fg-subtle" aria-label="Sent" />}
            </>
          ) : null}
        </span>
      )}
    </motion.li>
  );
});
