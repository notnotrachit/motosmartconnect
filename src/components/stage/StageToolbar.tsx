import { useState } from "react";
import { Camera, Monitor, ScreenShare, Square } from "lucide-react";
import { IconButton } from "@/components/ui/IconButton";
import { startDesktop, startMirror, startWebcam, stopStream } from "@/engine/actions";

interface StageToolbarProps {
  connected: boolean;
  canStop: boolean;
  mirroring: boolean;
}

type Busy = "" | "mirror" | "desktop" | "camera" | "stop";

/** Screen sources and the stop control, above the phone. */
export function StageToolbar({ connected, canStop, mirroring }: StageToolbarProps) {
  const [busy, setBusy] = useState<Busy>("");
  const run = (which: Busy, action: () => Promise<unknown>) => async () => {
    setBusy(which);
    try {
      await action();
    } finally {
      setBusy("");
    }
  };

  return (
    <div role="toolbar" aria-label="Screen sources" className="flex items-center gap-0.5 rounded-xl border border-line bg-surface/70 p-0.5">
      <IconButton
        label="Mirror screen"
        icon={<ScreenShare size={15} />}
        active={mirroring}
        disabled={!connected}
        loading={busy === "mirror"}
        onClick={run("mirror", startMirror)}
      />
      <IconButton
        label="Phone desktop (new window)"
        icon={<Monitor size={15} />}
        disabled={!connected}
        loading={busy === "desktop"}
        onClick={run("desktop", startDesktop)}
      />
      <IconButton
        label="Phone as camera (new window)"
        icon={<Camera size={15} />}
        disabled={!connected}
        loading={busy === "camera"}
        onClick={run("camera", startWebcam)}
      />
      <span className="mx-0.5 h-4 w-px bg-line" />
      <IconButton
        label="Stop screen"
        variant="danger"
        icon={<Square size={13} fill="currentColor" />}
        disabled={!canStop}
        loading={busy === "stop"}
        onClick={run("stop", stopStream)}
      />
    </div>
  );
}
