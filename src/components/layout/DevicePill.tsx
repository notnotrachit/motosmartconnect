import { AnimatePresence, motion } from "motion/react";
import { ChevronDown, Smartphone } from "lucide-react";
import { StatusDot } from "@/components/ui/Feedback";
import { useConnectedPhone, usePhones } from "@/store/engine";
import { useUi } from "@/store/ui";

/** Current phone and its link state. Opens the Phones section of Settings. */
export function DevicePill() {
  const phones = usePhones();
  const live = useConnectedPhone();
  const openSettings = useUi((s) => s.openSettings);
  const phone = live ?? phones[0];
  if (!phone) return null;

  const status = live ? "Connected" : phone.userDisconnected ? "Disconnected" : "Offline";
  return (
    <motion.button
      type="button"
      initial={{ opacity: 0, x: -6 }}
      animate={{ opacity: 1, x: 0 }}
      onClick={() => openSettings("phones")}
      className="group flex h-8 items-center gap-2 rounded-lg border border-line bg-surface/70 pl-2 pr-2 text-left transition hover:border-line-strong hover:bg-surface active:scale-[0.98]"
      aria-label={`${phone.name || "Phone"}, ${status}. Manage phones`}
    >
      <span className="relative grid size-5 place-items-center rounded-md bg-surface-2 text-fg-muted">
        <Smartphone size={12} />
        <StatusDot tone={live ? "live" : "off"} pulse={Boolean(live)} className="absolute -bottom-0.5 -right-0.5 ring-2 ring-surface rounded-full" />
      </span>
      <span className="max-w-[16ch] truncate text-xs font-medium text-fg">{phone.name || "Phone"}</span>
      <AnimatePresence mode="wait" initial={false}>
        <motion.span
          key={status}
          initial={{ opacity: 0, y: 3 }}
          animate={{ opacity: 1, y: 0 }}
          exit={{ opacity: 0, y: -3 }}
          transition={{ duration: 0.15 }}
          className={live ? "text-xs text-success" : "text-xs text-fg-subtle"}
        >
          {status}
        </motion.span>
      </AnimatePresence>
      <ChevronDown size={12} className="text-fg-subtle transition group-hover:text-fg-muted" />
    </motion.button>
  );
}
