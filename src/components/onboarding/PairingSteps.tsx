import { motion } from "motion/react";
import { cn } from "@/lib/cn";

const STEPS = [
  { title: "Open Smart Connect", body: "On your Motorola phone, open the Smart Connect app." },
  { title: "Add a PC", body: "Choose to connect a computer. Your phone will see this Mac as a PC." },
  { title: "Scan the code", body: "Point the phone's camera at the code. Both devices need the same Wi-Fi." },
];

export function PairingSteps({ compact = false, delay = 0 }: { compact?: boolean; delay?: number }) {
  return (
    <ol className={cn("flex flex-col", compact ? "gap-3" : "gap-4")}>
      {STEPS.map((step, index) => (
        <motion.li
          key={step.title}
          initial={{ opacity: 0, x: -8 }}
          animate={{ opacity: 1, x: 0 }}
          transition={{ delay: delay + index * 0.08, duration: 0.35, ease: [0.16, 1, 0.3, 1] }}
          className="flex gap-3"
        >
          <span
            className={cn(
              "grid shrink-0 place-items-center rounded-full bg-accent/15 font-semibold text-accent ring-1 ring-accent/25",
              compact ? "size-6 text-[11px]" : "size-7 text-xs",
            )}
          >
            {index + 1}
          </span>
          <div className="min-w-0 pt-0.5">
            <p className={cn("font-medium text-fg", compact ? "text-[0.8125rem]" : "text-sm")}>{step.title}</p>
            <p className={cn("mt-0.5 leading-relaxed text-fg-muted", compact ? "text-xs" : "text-[0.8125rem]")}>{step.body}</p>
          </div>
        </motion.li>
      ))}
    </ol>
  );
}
