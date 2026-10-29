import { useSyncExternalStore } from "react";
import { mirror } from "@/engine/mirror";
import type { ScreenState } from "@/rtc/MirrorController";

/** Live state of the phone-screen picture in the main window. */
export function useScreen(): ScreenState {
  const controller = mirror();
  return useSyncExternalStore(controller.subscribe, controller.getState);
}
