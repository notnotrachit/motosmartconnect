import { motion } from "motion/react";
import { Spinner } from "@/components/ui/Spinner";
import { BrandMark } from "./Brand";

/** Shown until the engine returns its first snapshot. */
export function BootScreen() {
  return (
    <motion.div
      initial={{ opacity: 0 }}
      animate={{ opacity: 1 }}
      exit={{ opacity: 0, transition: { duration: 0.2 } }}
      transition={{ delay: 0.15 }}
      className="absolute inset-0 grid place-items-center"
    >
      <div className="flex flex-col items-center gap-4 text-fg-muted">
        <BrandMark size={44} className="rounded-xl" />
        <div className="flex items-center gap-2 text-[0.8125rem]">
          <Spinner size={14} />
          Starting Smart Connect…
        </div>
      </div>
    </motion.div>
  );
}
