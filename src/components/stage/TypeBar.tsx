import { useState, type FormEvent } from "react";
import { AnimatePresence, motion } from "motion/react";
import { Check, CornerDownLeft, Keyboard } from "lucide-react";
import { Input } from "@/components/ui/Input";
import { typeOnPhone } from "@/engine/actions";
import { cn } from "@/lib/cn";

/** Send a whole string to the focused field on the phone. */
export function TypeBar({ disabled }: { disabled: boolean }) {
  const [text, setText] = useState("");
  const [sent, setSent] = useState(false);

  const submit = async (event: FormEvent) => {
    event.preventDefault();
    if (!text || disabled) return;
    const value = text;
    setText("");
    await typeOnPhone(value);
    setSent(true);
    window.setTimeout(() => setSent(false), 1200);
  };

  return (
    <form onSubmit={submit} className="w-full">
      <Input
        value={text}
        onChange={(event) => setText(event.target.value)}
        maxLength={4000}
        autoComplete="off"
        spellCheck={false}
        disabled={disabled}
        placeholder={disabled ? "Mirror the screen to type on the phone" : "Type on the phone…"}
        aria-label="Text to type on the phone"
        leading={<Keyboard size={14} />}
        wrapperClassName={cn("h-9", disabled && "opacity-60")}
        trailing={
          <button
            type="submit"
            aria-label="Send text"
            disabled={disabled || !text}
            className={cn(
              "grid size-6 place-items-center rounded-md transition-all duration-200",
              text ? "bg-accent text-on-accent" : "text-fg-subtle",
              "disabled:opacity-60",
            )}
          >
            <AnimatePresence mode="wait" initial={false}>
              <motion.span
                key={sent ? "sent" : "enter"}
                initial={{ scale: 0.4, opacity: 0 }}
                animate={{ scale: 1, opacity: 1 }}
                exit={{ scale: 0.4, opacity: 0 }}
                transition={{ type: "spring", stiffness: 600, damping: 26 }}
              >
                {sent ? <Check size={13} className="text-success" /> : <CornerDownLeft size={13} />}
              </motion.span>
            </AnimatePresence>
          </button>
        }
      />
    </form>
  );
}
