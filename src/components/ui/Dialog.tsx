import { useId, useRef, type ReactNode } from "react";
import { AnimatePresence, motion } from "motion/react";
import { X } from "lucide-react";
import { cn } from "@/lib/cn";
import { useFocusTrap } from "@/hooks/useFocusTrap";
import { useLayer } from "@/hooks/useLayer";
import { IconButton } from "./IconButton";

interface DialogProps {
  open: boolean;
  onClose: () => void;
  title?: ReactNode;
  description?: ReactNode;
  children: ReactNode;
  size?: "sm" | "md" | "lg" | "xl";
  /** "top" suits command menus; "center" suits sheets. */
  placement?: "center" | "top";
  hideHeader?: boolean;
  className?: string;
}

const SIZES = { sm: "max-w-sm", md: "max-w-md", lg: "max-w-2xl", xl: "max-w-4xl" };

/** Modal sheet with scrim, focus trap, Escape-to-close and spring motion. */
export function Dialog({
  open,
  onClose,
  title,
  description,
  children,
  size = "md",
  placement = "center",
  hideHeader = false,
  className,
}: DialogProps) {
  const titleId = useId();
  const panel = useRef<HTMLDivElement>(null);
  useLayer(open, onClose);
  useFocusTrap(panel, open);

  return (
    <AnimatePresence>
      {open && (
        <div
          className={cn(
            "fixed inset-0 z-50 flex justify-center p-6",
            placement === "center" ? "items-center" : "items-start pt-[12vh]",
          )}
        >
          <motion.div
            className="absolute inset-0 bg-black/35 backdrop-blur-[3px] dark:bg-black/55"
            initial={{ opacity: 0 }}
            animate={{ opacity: 1 }}
            exit={{ opacity: 0 }}
            transition={{ duration: 0.2 }}
            onClick={onClose}
          />
          <motion.div
            ref={panel}
            role="dialog"
            aria-modal="true"
            aria-labelledby={title ? titleId : undefined}
            tabIndex={-1}
            initial={{ opacity: 0, scale: 0.96, y: placement === "top" ? -8 : 10 }}
            animate={{ opacity: 1, scale: 1, y: 0 }}
            exit={{ opacity: 0, scale: 0.97, y: placement === "top" ? -4 : 6, transition: { duration: 0.14 } }}
            transition={{ type: "spring", duration: 0.38, bounce: 0.16 }}
            className={cn(
              "relative flex max-h-full w-full flex-col overflow-hidden rounded-2xl border border-line bg-surface shadow-pop outline-none",
              SIZES[size],
              className,
            )}
          >
            {!hideHeader && (title || description) && (
              <header className="flex items-start gap-4 border-b border-line px-5 pb-4 pt-5">
                <div className="min-w-0 flex-1">
                  {title && (
                    <h2 id={titleId} className="font-display text-[0.95rem] font-semibold tracking-tight text-fg">
                      {title}
                    </h2>
                  )}
                  {description && <p className="mt-1 text-[0.8125rem] text-fg-muted">{description}</p>}
                </div>
                <IconButton label="Close" shortcut="esc" icon={<X size={16} />} size="sm" onClick={onClose} className="-mr-1 -mt-0.5" />
              </header>
            )}
            {children}
          </motion.div>
        </div>
      )}
    </AnimatePresence>
  );
}
