import { invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import type {
  IconPack,
  InputArgs,
  MirrorView,
  PhoneEntry,
  RtcSignal,
  Snapshot,
} from "./types";

/** Typed wrappers for every command registered in src-tauri/src/lib.rs. */
export const api = {
  snapshot: () => invoke<Snapshot>("snapshot"),
  wallpaper: () => invoke<string>("wallpaper"),
  setName: (name: string) => invoke<Snapshot>("set_name", { name }),
  refreshQr: () => invoke<Snapshot>("refresh_qr"),
  beginPairing: () => invoke<Snapshot>("begin_pairing"),
  endPairing: () => invoke<Snapshot>("end_pairing"),
  unpair: (realId: string) => invoke<Snapshot>("unpair", { realId }),

  messagesOpen: (key: string) => invoke<Snapshot>("messages_open", { key }),
  messagesClose: () => invoke<Snapshot>("messages_close"),
  messagesSend: (text: string) => invoke<Snapshot>("messages_send", { text }),
  messagesSendNew: (address: string, text: string) => invoke<Snapshot>("messages_send_new", { address, text }),

  notificationReply: (key: string, text: string) => invoke<Snapshot>("notification_reply", { key, text }),
  notificationAct: (key: string, actionId: number) => invoke<Snapshot>("notification_act", { key, actionId }),
  notificationOpen: (key: string) => invoke<Snapshot>("notification_open", { key }),

  startMirror: () => invoke<Snapshot>("start_mirror"),
  startDesktop: () => invoke<Snapshot>("start_desktop"),
  startWebcam: () => invoke<Snapshot>("start_webcam"),
  webcamFlip: () => invoke<Snapshot>("webcam_flip"),
  startApp: (pkg: string, cmp: string, userId: number) => invoke<Snapshot>("start_app", { pkg, cmp, userId }),
  openStreamWindow: (pkg: string, title: string) => invoke<void>("open_stream_window", { pkg, title }),
  stopStream: (sessionId: string) => invoke<Snapshot>("stop_stream", { sessionId }),

  playDecoded: (sessionId: string, sdp: string) => invoke<void>("play_decoded", { sessionId, sdp }),
  pageOffers: () => invoke<RtcSignal[]>("page_offers"),
  webrtcAnswer: (sessionId: string, sdp: string) => invoke<void>("webrtc_answer", { sessionId, sdp }),
  webrtcIce: (item: { sessionId: string; candidate: string; label: number; mid: string }) =>
    invoke<void>("webrtc_ice", item),
  screenNote: (text: string) => invoke<void>("screen_note", { text }),
  mirrorFrame: (since: number) => invoke<MirrorView>("mirror_frame", { since }),

  crossStart: () => invoke<Snapshot>("cross_start"),
  crossStop: () => invoke<Snapshot>("cross_stop"),
  crossShow: (show: boolean) => invoke<void>("cross_show", { show }),
  sendInput: (args: InputArgs) => invoke<void>("send_input", { ...args }),

  refreshApps: () => invoke<Snapshot>("refresh_apps"),
  appIcons: () => invoke<IconPack>("app_icons"),
  revealFile: (path: string) => invoke<void>("reveal_file", { path }),
  listPhone: (path: string) => invoke<PhoneEntry[]>("list_phone", { path }),
  savePhone: (path: string) => invoke<string>("save_phone", { path }),
};

export const events = {
  onSnapshot: (fn: (snapshot: Snapshot) => void): Promise<UnlistenFn> =>
    listen<Snapshot>("snapshot", (event) => fn(event.payload)),
  onRtc: (fn: (signal: RtcSignal) => void): Promise<UnlistenFn> =>
    listen<RtcSignal>("rtc", (event) => fn(event.payload)),
  onFrame: (fn: (frame: MirrorView) => void): Promise<UnlistenFn> =>
    listen<MirrorView>("frame", (event) => fn(event.payload)),
  onWallpaper: (fn: (url: string) => void): Promise<UnlistenFn> =>
    listen<string>("wallpaper", (event) => fn(event.payload)),
};

/** Tauri rejects with plain strings; normalize anything thrown into readable text. */
export function errorText(error: unknown): string {
  if (typeof error === "string") return error;
  if (error instanceof Error) return error.message;
  if (error && typeof error === "object" && "message" in error) return String((error as { message: unknown }).message);
  return String(error);
}
