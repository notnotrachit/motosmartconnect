import { useState } from "react";
import { Info, MousePointer2, Power } from "lucide-react";
import { Button } from "@/components/ui/Button";
import { StatusDot } from "@/components/ui/Feedback";
import { crossStart, crossStop } from "@/engine/actions";
import { cn } from "@/lib/cn";
import { useConnectedPhone, useEngine } from "@/store/engine";
import { PanelHeader } from "../PanelHeader";
import { CrossPad } from "./CrossPad";

const OFF = { active: false, width: 0, height: 0 };

export function ControlPanel() {
  const cross = useEngine((s) => s.snapshot?.cross) ?? OFF;
  const connected = Boolean(useConnectedPhone());
  const [busy, setBusy] = useState(false);

  const toggle = async () => {
    setBusy(true);
    await (cross.active ? crossStop() : crossStart());
    setBusy(false);
  };

  return (
    <>
      <PanelHeader title="Cross control" subtitle="Use this Mac's mouse and keyboard on the phone" />
      <div className="flex min-h-0 flex-1 flex-col gap-4 px-6 pb-6">
        <div
          className={cn(
            "flex items-center gap-4 rounded-2xl border p-4 transition-colors duration-300",
            cross.active ? "border-accent/30 bg-accent/[0.07]" : "border-line bg-surface",
          )}
        >
          <span
            className={cn(
              "grid size-10 shrink-0 place-items-center rounded-xl transition-colors duration-300",
              cross.active ? "bg-accent text-on-accent" : "bg-surface-2 text-fg-muted",
            )}
          >
            <MousePointer2 size={18} />
          </span>
          <div className="min-w-0 flex-1">
            <p className="flex items-center gap-2 text-[0.8125rem] font-semibold text-fg">
              {cross.active ? "Controlling the phone" : "Cross control is off"}
              <StatusDot tone={cross.active ? "live" : "off"} pulse={cross.active} />
            </p>
            <p className="mt-0.5 text-xs text-fg-muted">
              {cross.active
                ? "Move over the pad below. Clicks, scrolls and keys go to the phone."
                : "Start here, or tap Cross control on the phone. The phone may ask you to allow this Mac."}
            </p>
          </div>
          <Button
            variant={cross.active ? "danger" : "primary"}
            icon={<Power size={14} />}
            loading={busy}
            disabled={!connected && !cross.active}
            onClick={toggle}
          >
            {cross.active ? "Stop" : "Start control"}
          </Button>
        </div>

        <CrossPad cross={cross} />

        <p className="flex items-center justify-center gap-1.5 text-[11px] text-fg-subtle">
          <Info size={12} />
          Leaving the pad hides the pointer on the phone.
        </p>
      </div>
    </>
  );
}
