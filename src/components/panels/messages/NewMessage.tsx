import { useMemo } from "react";
import { ChevronLeft, X } from "lucide-react";
import { Avatar } from "@/components/ui/Avatar";
import { IconButton } from "@/components/ui/IconButton";
import { sendNewThread } from "@/engine/actions";
import { cn } from "@/lib/cn";
import type { ThreadView } from "@/lib/types";
import { Composer } from "./Composer";
import { isBareNumber, validNumber } from "./format";
import { NEW_DRAFT, useMessageUi } from "@/store/messages";

interface NewMessageProps {
  threads: ThreadView[];
  canSend: boolean;
  onOpenThread: (key: string) => void;
}

/** Compose to a number. Suggests existing conversations while typing. */
export function NewMessage({ threads, canSend, onOpenThread }: NewMessageProps) {
  const to = useMessageUi((s) => s.to);
  const setTo = useMessageUi((s) => s.setTo);
  const text = useMessageUi((s) => s.drafts[NEW_DRAFT] ?? "");
  const setDraft = useMessageUi((s) => s.setDraft);
  const stop = useMessageUi((s) => s.stopCompose);
  const valid = validNumber(to);
  const showError = to.trim().length > 0 && !/^[\d+\-() ]*$/.test(to.trim());

  const suggestions = useMemo(() => {
    const q = to.trim().toLowerCase();
    const digits = q.replace(/\D/g, "");
    if (!q) return [];
    return threads
      .filter((t) => t.title.toLowerCase().includes(q) || (digits.length >= 3 && t.address.replace(/\D/g, "").includes(digits)))
      .slice(0, 5);
  }, [threads, to]);

  const send = async (body: string) => {
    const ok = await sendNewThread(to, body);
    if (ok) {
      setTo("");
      setDraft(NEW_DRAFT, "");
      stop();
    }
    return ok;
  };

  const pick = (thread: ThreadView) => {
    // Carry the typed text into the existing conversation.
    if (text) setDraft(thread.key, text);
    setDraft(NEW_DRAFT, "");
    setTo("");
    stop();
    onOpenThread(thread.key);
  };

  return (
    <div className="flex min-h-0 flex-1 flex-col">
      <header className="flex h-[60px] shrink-0 items-center gap-3 border-b border-line px-4">
        <span className="@2xl:hidden">
          <IconButton label="Back to conversations" icon={<ChevronLeft size={16} />} size="sm" onClick={stop} />
        </span>
        <h2 className="flex-1 text-[0.8125rem] font-semibold text-fg">New message</h2>
        <span className="hidden @2xl:inline-flex">
          <IconButton label="Discard" shortcut="esc" icon={<X size={15} />} size="sm" onClick={stop} />
        </span>
      </header>

      <div className="relative shrink-0 border-b border-line px-4 py-2">
        <label className="flex items-center gap-2 text-[0.8125rem]">
          <span className="text-fg-subtle">To:</span>
          <input
            autoFocus
            value={to}
            onChange={(event) => setTo(event.target.value)}
            onKeyDown={(event) => {
              if (event.key === "Escape") stop();
              if (event.key === "Enter" && suggestions[0]) {
                event.preventDefault();
                pick(suggestions[0]);
              }
            }}
            inputMode="tel"
            autoComplete="off"
            placeholder="Phone number or a contact you've texted"
            aria-label="Recipient"
            aria-invalid={showError || undefined}
            className="h-8 flex-1 bg-transparent text-fg outline-none placeholder:text-fg-subtle focus-visible:outline-none"
          />
          {valid && <span className="rounded-full bg-success/12 px-2 py-0.5 text-[10.5px] font-medium text-success">Valid number</span>}
        </label>
        {showError && <p className="pb-1 text-[11px] text-danger">Use digits, spaces, +, - or brackets.</p>}
      </div>

      <div className="min-h-0 flex-1 overflow-y-auto px-2 py-2">
        {suggestions.length > 0 ? (
          <>
            <p className="px-3 pb-1 pt-1 text-[10.5px] font-semibold uppercase tracking-wider text-fg-subtle">Existing conversations</p>
            <ul>
              {suggestions.map((thread, i) => (
                <li key={thread.key}>
                  <button
                    type="button"
                    onClick={() => pick(thread)}
                    className={cn(
                      "flex w-full items-center gap-3 rounded-xl px-3 py-2 text-left transition-colors hover:bg-surface-2",
                      i === 0 && "bg-surface-2/60",
                    )}
                  >
                    <Avatar name={thread.title} anonymous={isBareNumber(thread.title)} size={30} />
                    <span className="min-w-0 flex-1">
                      <span className="block truncate text-[0.8125rem] font-medium text-fg">{thread.title}</span>
                      <span className="block truncate font-mono text-[11px] text-fg-subtle">{thread.address}</span>
                    </span>
                    {i === 0 && <span className="text-[10.5px] text-fg-subtle">↵</span>}
                  </button>
                </li>
              ))}
            </ul>
          </>
        ) : (
          <p className="px-3 py-8 text-center text-xs text-fg-subtle">
            {to.trim() ? (valid ? "This starts a new conversation." : "Keep typing the number.") : "Type a number to start a conversation."}
          </p>
        )}
      </div>

      <Composer
        value={text}
        onChange={(value) => setDraft(NEW_DRAFT, value)}
        onSend={send}
        ready={valid}
        disabled={!canSend}
        placeholder={valid ? "Text message" : "Add a recipient first"}
        onEscape={text ? undefined : stop}
      />
    </div>
  );
}
