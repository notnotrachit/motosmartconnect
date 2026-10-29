import { SCREEN_IDLE, SCREEN_OFFLINE, type ScreenState } from "@/rtc/MirrorController";

export type ScreenPhase = "live" | "idle" | "offline" | "busy" | "error";

const FAILURE = /fail|did not|could not|rejected|no usable|no LAN|declined|stopped/i;

/** Collapse the controller's free-text status into a phase the UI can design for. */
export function screenPhase(screen: ScreenState, connected: boolean): ScreenPhase {
  if (screen.live) return "live";
  if (screen.label === SCREEN_OFFLINE || (!connected && screen.label === SCREEN_IDLE)) return "offline";
  if (screen.label === SCREEN_IDLE) return "idle";
  if (FAILURE.test(screen.label)) return "error";
  return "busy";
}
