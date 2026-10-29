import { create } from "zustand";
import type { IconPack, PhoneView, SmsView, Snapshot } from "@/lib/types";

/**
 * The latest engine snapshot plus a few derived bits that would otherwise be
 * recomputed in every component. Only engine/sync.ts writes to this store.
 */
interface EngineStore {
  snapshot: Snapshot | null;
  icons: IconPack;
  /** Wall-clock time (ms) when the visible QR code expires. */
  qrDeadline: number;
  /** Lifetime (s) of the visible QR code, for the countdown bar. */
  qrLifetime: number;
  /** Phone wallpaper as a data URL. Empty until the phone sends one. */
  wallpaper: string;
}

export const useEngine = create<EngineStore>(() => ({
  snapshot: null,
  icons: { revision: -1, icons: {} },
  qrDeadline: 0,
  qrLifetime: 0,
  wallpaper: "",
}));

const EMPTY: never[] = [];

// Selectors. Each returns a value that is referentially stable per snapshot.
export const useSnapshot = () => useEngine((s) => s.snapshot);
export const usePhones = () => useEngine((s) => s.snapshot?.phones ?? (EMPTY as PhoneView[]));
export const useConnectedPhone = () => useEngine((s) => s.snapshot?.phones?.find((p) => p.connected) ?? null);
export const usePaired = () => useEngine((s) => (s.snapshot?.phones?.length ?? 0) > 0);
export const useNotifications = () => useEngine((s) => s.snapshot?.notifications ?? EMPTY);
const EMPTY_SMS: SmsView = { phase: "waiting", unread: 0, threads: [], openKey: "", openTitle: "", openAddress: "", open: [] };
export const useMessages = () => useEngine((s) => s.snapshot?.messages ?? EMPTY_SMS);
export const useApps = () => useEngine((s) => s.snapshot?.apps ?? EMPTY);
export const useFiles = () => useEngine((s) => s.snapshot?.files ?? EMPTY);
export const useSessions = () => useEngine((s) => s.snapshot?.sessions ?? EMPTY);
export const useLog = () => useEngine((s) => s.snapshot?.log ?? (EMPTY as string[]));
export const useIcons = () => useEngine((s) => s.icons.icons);
export const useWallpaper = () => useEngine((s) => s.wallpaper);

/** Icon data URL for an app package, or "". */
export function iconFor(pkg: string, snapshot: Snapshot | null, icons: Record<string, string>): string {
  const app = (snapshot?.apps ?? []).find((item) => item.pkg === pkg);
  return (app?.iconName && icons[app.iconName]) || "";
}
