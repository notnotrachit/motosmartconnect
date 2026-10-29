import { motion } from "motion/react";
import { ShieldCheck, Sparkles } from "lucide-react";
import { PairingSteps } from "./PairingSteps";
import { QrCard } from "./QrCard";

/** First-run screen shown until a phone is paired. */
export function Onboarding() {
  return (
    <motion.div
      initial={{ opacity: 0 }}
      animate={{ opacity: 1 }}
      exit={{ opacity: 0, scale: 0.99, transition: { duration: 0.2 } }}
      className="absolute inset-0 overflow-auto"
    >
      {/* Ambient backdrop */}
      <div aria-hidden="true" className="pointer-events-none absolute inset-0 overflow-hidden">
        <div className="absolute -left-40 -top-40 size-[560px] rounded-full bg-accent/15 blur-3xl" />
        <div className="absolute -bottom-48 right-[-10%] size-[520px] rounded-full bg-accent/10 blur-3xl" />
        <div className="absolute inset-0 [background-image:radial-gradient(var(--line-strong)_1px,transparent_1px)] [background-size:22px_22px] [mask-image:radial-gradient(ellipse_at_center,black_30%,transparent_75%)]" />
      </div>

      <div className="relative mx-auto flex min-h-full max-w-5xl items-center gap-16 px-10 py-12">
        <div className="min-w-0 flex-1">
          <motion.span
            initial={{ opacity: 0, y: 6 }}
            animate={{ opacity: 1, y: 0 }}
            className="inline-flex items-center gap-1.5 rounded-full border border-accent/25 bg-accent/10 px-2.5 py-1 text-xs font-medium text-accent"
          >
            <Sparkles size={12} />
            Get started
          </motion.span>
          <motion.h1
            initial={{ opacity: 0, y: 10 }}
            animate={{ opacity: 1, y: 0 }}
            transition={{ delay: 0.05, duration: 0.45, ease: [0.16, 1, 0.3, 1] }}
            className="mt-4 max-w-[14ch] font-display text-[2.6rem] font-semibold leading-[1.05] tracking-[-0.03em] text-fg"
          >
            Connect your phone to this Mac.
          </motion.h1>
          <motion.p
            initial={{ opacity: 0, y: 10 }}
            animate={{ opacity: 1, y: 0 }}
            transition={{ delay: 0.1, duration: 0.45, ease: [0.16, 1, 0.3, 1] }}
            className="mt-4 max-w-[46ch] text-[0.95rem] leading-relaxed text-fg-muted"
          >
            Mirror the screen, answer notifications, open apps, move files and use the camera — all from your desktop.
          </motion.p>

          <div className="mt-8">
            <PairingSteps delay={0.15} />
          </div>

          <motion.p
            initial={{ opacity: 0 }}
            animate={{ opacity: 1 }}
            transition={{ delay: 0.5 }}
            className="mt-8 flex items-center gap-2 text-xs text-fg-subtle"
          >
            <ShieldCheck size={14} />
            Pairing stays on your local network. The phone pins this Mac's certificate.
          </motion.p>
        </div>

        <motion.div
          initial={{ opacity: 0, y: 16, scale: 0.97 }}
          animate={{ opacity: 1, y: 0, scale: 1 }}
          transition={{ delay: 0.1, type: "spring", duration: 0.6, bounce: 0.2 }}
          className="w-[340px] shrink-0"
        >
          <QrCard />
        </motion.div>
      </div>
    </motion.div>
  );
}
