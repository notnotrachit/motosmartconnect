import { useState } from "react";
import { AnimatePresence, motion } from "motion/react";
import { Check, Copy } from "lucide-react";
import { IconButton } from "@/components/ui/IconButton";
import { toast } from "@/store/toast";

/** Copies `value` and flips to a check mark for a moment. */
export function CopyButton({ value, label = "Copy" }: { value: string; label?: string }) {
  const [copied, setCopied] = useState(false);
  const copy = async () => {
    try {
      await navigator.clipboard.writeText(value);
      setCopied(true);
      window.setTimeout(() => setCopied(false), 1400);
    } catch {
      toast.error("Couldn't copy to the clipboard");
    }
  };
  return (
    <IconButton
      label={copied ? "Copied" : label}
      size="sm"
      onClick={copy}
      icon={
        <AnimatePresence mode="wait" initial={false}>
          <motion.span
            key={copied ? "y" : "n"}
            initial={{ scale: 0.5, opacity: 0 }}
            animate={{ scale: 1, opacity: 1 }}
            exit={{ scale: 0.5, opacity: 0 }}
            transition={{ duration: 0.15 }}
          >
            {copied ? <Check size={13} className="text-success" /> : <Copy size={13} />}
          </motion.span>
        </AnimatePresence>
      }
    />
  );
}
