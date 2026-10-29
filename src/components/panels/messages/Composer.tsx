import { useEffect, useLayoutEffect, useRef, useState, type FormEvent, type KeyboardEvent } from "react";
import { AnimatePresence, motion } from "motion/react";
import { ArrowUp } from "lucide-react";
import { Spinner } from "@/components/ui/Spinner";
import { cn } from "@/lib/cn";
import { smsParts } from "./format";
import { useMessageUi } from "@/store/messages";

interface ComposerProps {
  value: string;
  onChange: (value: string) => void;
  /** Resolves true when the engine accepted the text. */
  onSend: (text: string) => Promise<boolean>;
  disabled?: boolean;
  placeholder?: string;
  /** Extra condition the send button needs (e.g. a valid recipient). */
  ready?: boolean;
  autoFocus?: boolean;
  onEscape?: () => void;
}

const MAX_HEIGHT = 140;

/** Growing text field. Enter sends, Shift-Enter adds a line. */
export function Composer({ value, onChange, onSend, disabled, placeholder, ready = true, autoFocus, onEscape }: ComposerProps) {
  const field = useRef<HTMLTextAreaElement>(null);
  const [sending, setSending] = useState(false);
  const focusTick = useMessageUi((s) => s.focusTick);
  const canSend = !disabled && ready && value.trim().length > 0 && !sending;
  const { parts, left } = smsParts(value);

  // Size to content.
  useLayoutEffect(() => {
    const el = field.current;
    if (!el) return;
    el.style.height = "0px";
    el.style.height = `${Math.min(MAX_HEIGHT, el.scrollHeight)}px`;
  }, [value]);

  useEffect(() => {
    if (focusTick && !disabled) field.current?.focus();
  }, [focusTick, disabled]);

  const send = async (event?: FormEvent) => {
    event?.preventDefault();
    if (!canSend) return;
    setSending(true);
    const ok = await onSend(value.trim());
    setSending(false);
    if (ok) onChange("");
    field.current?.focus();
  };

  const onKeyDown = (event: KeyboardEvent<HTMLTextAreaElement>) => {
    if (event.key === "Enter" && !event.shiftKey && !event.nativeEvent.isComposing) {
      event.preventDefault();
      void send();
    } else if (event.key === "Escape") {
      // Handled here, so the conversation doesn't also close on it.
      event.preventDefault();
      event.stopPropagation();
      if (onEscape) onEscape();
      else field.current?.blur();
    }
  };

  return (
    <form onSubmit={send} className="shrink-0 border-t border-line bg-canvas/80 px-4 py-3 backdrop-blur">
      <div
        className={cn(
          "flex items-end gap-2 rounded-[20px] border border-line bg-surface py-1.5 pl-3.5 pr-1.5 transition-[border-color,box-shadow] duration-150",
          "focus-within:border-accent/50 focus-within:ring-[3px] focus-within:ring-accent/15",
          disabled && "opacity-60",
        )}
      >
        <textarea
          ref={field}
          rows={1}
          value={value}
          autoFocus={autoFocus}
          disabled={disabled}
          onChange={(event) => onChange(event.target.value)}
          onKeyDown={onKeyDown}
          placeholder={placeholder}
          aria-label={placeholder ?? "Text message"}
          maxLength={2000}
          spellCheck
          className="max-h-[140px] min-h-[22px] flex-1 resize-none self-center bg-transparent py-1 text-[0.8125rem] leading-[1.45] text-fg outline-none placeholder:text-fg-subtle focus-visible:outline-none"
        />
        <button
          type="submit"
          aria-label="Send"
          disabled={!canSend}
          className={cn(
            "grid size-7 shrink-0 place-items-center rounded-full transition-[background-color,color,transform,opacity] duration-200 ease-spring",
            canSend || sending ? "scale-100 bg-accent text-on-accent hover:bg-accent-strong active:scale-90" : "scale-90 bg-surface-3 text-fg-subtle",
          )}
        >
          {sending ? <Spinner size={13} /> : <ArrowUp size={15} strokeWidth={2.6} />}
        </button>
      </div>
      <div className="mt-1.5 flex h-4 items-center justify-between px-1 text-[10.5px] text-fg-subtle">
        <span>
          <span className="font-medium text-fg-muted">↵</span> send · <span className="font-medium text-fg-muted">⇧↵</span> new line
        </span>
        <AnimatePresence>
          {value.length > 100 && (
            <motion.span
              initial={{ opacity: 0, y: 3 }}
              animate={{ opacity: 1, y: 0 }}
              exit={{ opacity: 0 }}
              className={cn("tabular-nums", parts > 1 && "text-warning")}
            >
              {parts > 1 ? `${parts} texts · ` : ""}
              {left} left
            </motion.span>
          )}
        </AnimatePresence>
      </div>
    </form>
  );
}
