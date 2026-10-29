import { Dialog } from "@/components/ui/Dialog";
import { endPairing } from "@/engine/actions";
import { useEngine, usePaired } from "@/store/engine";
import { PairingSteps } from "./PairingSteps";
import { QrCard } from "./QrCard";

/** "Pair another phone" sheet. Open while the engine has a pairing code out. */
export function PairDialog() {
  const paired = usePaired();
  const pairing = useEngine((s) => s.snapshot?.pairing ?? false);

  return (
    <Dialog
      open={paired && pairing}
      onClose={() => void endPairing()}
      size="lg"
      title="Pair another phone"
      description="Scan this code in Smart Connect on the new phone."
    >
      <div className="flex gap-8 overflow-auto p-6">
        <div className="min-w-0 flex-1 pt-1">
          <PairingSteps compact />
        </div>
        <QrCard className="w-[300px] shrink-0 shadow-none" />
      </div>
    </Dialog>
  );
}
