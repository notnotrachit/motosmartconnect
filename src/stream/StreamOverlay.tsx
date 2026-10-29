import { Camera, Monitor, Smartphone } from "lucide-react";
import { cn } from "@/lib/cn";
import type { StreamKind } from "@/rtc/StreamController";

interface StreamOverlayProps {
  kind: StreamKind;
  title: string;
  status: string;
  picture: boolean;
}

const ICONS = { app: Smartphone, desktop: Monitor, webcam: Camera };

/**
 * Loading card over the picture. It is absolutely positioned and only toggles
 * opacity/visibility, so the video underneath never moves or re-lays out.
 */
export function StreamOverlay({ kind, title, status, picture }: StreamOverlayProps) {
  const Icon = ICONS[kind];
  return (
    <div
      aria-hidden={picture}
      className={cn(
        "pointer-events-none absolute inset-0 z-[3] flex flex-col items-center justify-center px-8 text-center transition-[opacity,visibility] duration-500",
        "bg-[radial-gradient(80%_60%_at_50%_35%,color-mix(in_srgb,var(--accent)_14%,transparent),transparent_70%)]",
        picture ? "invisible opacity-0" : "visible opacity-100",
      )}
    >
      <div className="relative mb-5 grid size-14 place-items-center">
        <svg viewBox="0 0 56 56" className="absolute inset-0 size-14 animate-spin-fast text-accent">
          <circle cx="28" cy="28" r="25" fill="none" stroke="currentColor" strokeOpacity="0.15" strokeWidth="2.5" />
          <path d="M53 28A25 25 0 0 0 28 3" fill="none" stroke="currentColor" strokeWidth="2.5" strokeLinecap="round" />
        </svg>
        <Icon size={20} className="text-white/80" />
      </div>
      <p className="text-sm font-semibold">{title}</p>
      <p aria-live="polite" className="mt-1.5 line-clamp-3 max-w-[32ch] text-xs leading-relaxed text-white/55">
        {status}
      </p>
    </div>
  );
}
