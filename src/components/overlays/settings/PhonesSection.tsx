import { useState } from "react";
import { AnimatePresence, motion } from "motion/react";
import { Plus, Smartphone, Trash2 } from "lucide-react";
import { Button } from "@/components/ui/Button";
import { EmptyState, StatusDot } from "@/components/ui/Feedback";
import { beginPairing, forgetPhone } from "@/engine/actions";
import { confirm } from "@/store/confirm";
import { usePhones } from "@/store/engine";
import { useUi } from "@/store/ui";
import type { PhoneView } from "@/lib/types";

export function PhonesSection() {
  const phones = usePhones();
  const closeSettings = useUi((s) => s.closeSettings);

  const pairAnother = () => {
    closeSettings();
    void beginPairing();
  };

  return (
    <>
      <div className="mb-4 flex items-center justify-between">
        <p className="text-xs text-fg-muted">
          {phones.length ? `${phones.length} paired ${phones.length === 1 ? "phone" : "phones"}` : "No phones yet"}
        </p>
        <Button variant="primary" size="sm" icon={<Plus size={13} />} onClick={pairAnother}>
          Pair another phone
        </Button>
      </div>

      {!phones.length ? (
        <EmptyState icon={<Smartphone size={20} />} title="No paired phones" description="Pair a phone to use notifications, apps and the screen." />
      ) : (
        <ul className="flex flex-col gap-2">
          <AnimatePresence initial={false}>
            {phones.map((phone) => (
              <PhoneRow key={phone.realId} phone={phone} />
            ))}
          </AnimatePresence>
        </ul>
      )}
    </>
  );
}

function PhoneRow({ phone }: { phone: PhoneView }) {
  const [busy, setBusy] = useState(false);
  const confirmForget = useUi((s) => s.prefs.confirmForget);
  const name = phone.name || "Phone";
  const status = phone.connected ? "Connected" : phone.userDisconnected ? "Disconnected on the phone" : "Saved · reconnects when Smart Connect opens";

  const forget = async () => {
    if (confirmForget) {
      const ok = await confirm({
        title: `Forget ${name}?`,
        body: "This Mac stops trusting the phone. You'll need to scan a new code to pair it again.",
        confirmLabel: "Forget phone",
        tone: "danger",
      });
      if (!ok) return;
    }
    setBusy(true);
    await forgetPhone(phone.realId);
    setBusy(false);
  };

  return (
    <motion.li
      layout
      initial={{ opacity: 0, y: 6 }}
      animate={{ opacity: 1, y: 0 }}
      exit={{ opacity: 0, x: -16, transition: { duration: 0.15 } }}
      className="flex items-center gap-3 rounded-xl border border-line bg-surface-2/40 p-3"
    >
      <span className="relative grid size-10 place-items-center rounded-xl bg-surface-3 text-fg-muted">
        <Smartphone size={18} />
        <StatusDot tone={phone.connected ? "live" : "off"} className="absolute -bottom-0.5 -right-0.5 rounded-full ring-2 ring-surface" />
      </span>
      <div className="min-w-0 flex-1">
        <p className="truncate text-[0.8125rem] font-medium text-fg">{name}</p>
        <p className={phone.connected ? "text-xs text-success" : "text-xs text-fg-muted"}>{status}</p>
      </div>
      <Button variant="ghost" size="sm" icon={<Trash2 size={12} />} loading={busy} onClick={forget} className="hover:text-danger">
        Forget
      </Button>
    </motion.li>
  );
}
