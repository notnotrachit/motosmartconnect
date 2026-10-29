import { useMemo } from "react";
import { useScreen } from "@/hooks/useScreen";
import { useConnectedPhone, useSessions } from "@/store/engine";
import { AndroidKeys } from "./AndroidKeys";
import { ScreenWell } from "./ScreenWell";
import { SessionChips } from "./SessionChips";
import { StageToolbar } from "./StageToolbar";
import { TypeBar } from "./TypeBar";

/** Left column: the mirrored phone, its sources, navigation keys and text input. */
export function PhoneStage() {
  const screen = useScreen();
  const connected = Boolean(useConnectedPhone());
  const allSessions = useSessions();
  const sessions = useMemo(() => allSessions.filter((s) => !s.quiet), [allSessions]);
  const canStop = screen.live || Boolean(screen.activeSession) || sessions.length > 0;

  return (
    <section
      aria-label="Phone screen"
      className="flex w-[clamp(300px,31vw,420px)] shrink-0 flex-col border-r border-line bg-sidebar/50"
    >
      <div className="flex items-center justify-between gap-2 px-4 pb-1 pt-3">
        <div>
          <h2 className="text-[0.8125rem] font-semibold text-fg">Screen</h2>
          <p className="text-[11px] text-fg-subtle">
            {screen.live ? "Live from your phone" : connected ? "Ready to mirror" : "Waiting for the phone"}
          </p>
        </div>
        <StageToolbar connected={connected} canStop={canStop} mirroring={screen.live} />
      </div>

      {/* container-type lets the well size itself to the free space. */}
      <div className="flex min-h-0 flex-1 items-center justify-center px-5 py-3 [container-type:size]">
        <ScreenWell />
      </div>

      <div className="flex flex-col items-center gap-3 px-4 pb-4">
        {sessions.length > 1 && <SessionChips sessions={sessions} active={screen.activeSession} />}
        <AndroidKeys disabled={!screen.canInput} />
        <TypeBar disabled={!screen.canInput} />
      </div>
    </section>
  );
}
