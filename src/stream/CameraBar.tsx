import { useState } from "react";
import { SwitchCamera } from "lucide-react";
import { cn } from "@/lib/cn";

/** Footer for the phone-camera window. */
export function CameraBar({ picture, onFlip }: { picture: boolean; onFlip: () => void }) {
  const [turns, setTurns] = useState(0);
  return (
    <div className="flex h-11 shrink-0 items-center justify-between bg-black px-3 text-xs text-white/70">
      <span className="flex items-center gap-2">
        <span className={cn("size-1.5 rounded-full transition-colors", picture ? "animate-pulse bg-red-500" : "bg-white/30")} />
        {picture ? "Live · also sent to OBS Virtual Camera" : "Starting camera…"}
      </span>
      <button
        type="button"
        onClick={() => {
          setTurns((t) => t + 1);
          onFlip();
        }}
        className="flex items-center gap-1.5 rounded-lg px-2.5 py-1.5 text-white/80 transition hover:bg-white/10 hover:text-white active:scale-95"
      >
        <SwitchCamera size={14} className="transition-transform duration-500 ease-spring" style={{ transform: `rotate(${turns * 180}deg)` }} />
        Flip camera
      </button>
    </div>
  );
}
