import { useState, type FormEvent } from "react";
import { motion } from "motion/react";
import { SendHorizontal } from "lucide-react";
import { Button } from "@/components/ui/Button";
import { Input } from "@/components/ui/Input";
import { replyToNotice } from "@/engine/actions";

interface ReplyComposerProps {
  noticeKey: string;
  to: string;
  onDone: () => void;
}

/** Inline reply field that expands under a notification. */
export function ReplyComposer({ noticeKey, to, onDone }: ReplyComposerProps) {
  const [text, setText] = useState("");
  const [sending, setSending] = useState(false);

  const submit = async (event: FormEvent) => {
    event.preventDefault();
    if (!text.trim()) return;
    setSending(true);
    const ok = await replyToNotice(noticeKey, text);
    setSending(false);
    if (ok) {
      setText("");
      onDone();
    }
  };

  return (
    <motion.form
      onSubmit={submit}
      initial={{ height: 0, opacity: 0 }}
      animate={{ height: "auto", opacity: 1 }}
      exit={{ height: 0, opacity: 0 }}
      transition={{ duration: 0.22, ease: [0.16, 1, 0.3, 1] }}
      className="overflow-hidden"
      onClick={(event) => event.stopPropagation()}
    >
      <div className="flex gap-2 pt-3">
        <Input
          autoFocus
          value={text}
          onChange={(event) => setText(event.target.value)}
          onKeyDown={(event) => {
            if (event.key === "Escape") {
              event.stopPropagation();
              onDone();
            }
          }}
          maxLength={2000}
          autoComplete="off"
          placeholder={`Reply to ${to}…`}
          aria-label={`Reply to ${to}`}
          wrapperClassName="flex-1"
        />
        <Button type="submit" variant="primary" loading={sending} disabled={!text.trim()} icon={<SendHorizontal size={13} />}>
          Send
        </Button>
      </div>
    </motion.form>
  );
}
