import { useEffect, useRef, useSyncExternalStore } from "react";
import { BackGlyph, HomeGlyph, RecentGlyph } from "@/components/stage/glyphs";
import { ANDROID_KEYS } from "@/lib/keymap";
import type { StreamController } from "@/rtc/StreamController";
import { StreamOverlay } from "./StreamOverlay";
import { CameraBar } from "./CameraBar";

/** One phone app, the phone desktop, or the phone camera in its own window. */
export function StreamApp({ controller }: { controller: StreamController }) {
  const state = useSyncExternalStore(controller.subscribe, controller.getState);
  const well = useRef<HTMLDivElement>(null);
  const host = useRef<HTMLDivElement>(null);

  useEffect(() => {
    if (!well.current || !host.current) return;
    controller.mount(well.current, host.current);
    controller.start();
  }, [controller]);

  return (
    <div className="flex h-full flex-col bg-black text-white">
      <div
        ref={well}
        tabIndex={0}
        className="relative min-h-0 flex-[1_1_auto] touch-none select-none overflow-hidden bg-black outline-none"
      >
        {/* The controller owns this node's children. */}
        <div ref={host} className="h-full w-full" />
        <StreamOverlay kind={controller.kind} title={state.title} status={state.status} picture={state.picture} />
      </div>

      {controller.kind === "webcam" && <CameraBar picture={state.picture} onFlip={() => controller.flipCamera()} />}
      {controller.kind === "app" && (
        <nav aria-label="Android navigation" className="grid h-[52px] shrink-0 grid-cols-3 items-center bg-black">
          {[
            { label: "Back", code: ANDROID_KEYS.back, glyph: <BackGlyph /> },
            { label: "Home", code: ANDROID_KEYS.home, glyph: <HomeGlyph /> },
            { label: "Recent apps", code: ANDROID_KEYS.recent, glyph: <RecentGlyph /> },
          ].map((key) => (
            <button
              key={key.label}
              type="button"
              aria-label={key.label}
              onClick={() => controller.pressAndroidKey(key.code)}
              className="mx-auto grid size-11 place-items-center rounded-full text-white/80 transition-[background-color,color,transform] duration-150 hover:bg-white/10 hover:text-white active:scale-90 active:bg-white/20"
            >
              {key.glyph}
            </button>
          ))}
        </nav>
      )}
    </div>
  );
}
