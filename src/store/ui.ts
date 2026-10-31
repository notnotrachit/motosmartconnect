import { create } from "zustand";
import { persist } from "zustand/middleware";

export type Tab = "notifications" | "apps" | "files" | "control" | "messages";
export const TABS: Tab[] = ["notifications", "messages", "apps", "files", "control"];

export type ThemePref = "system" | "light" | "dark";
export type Accent = "green" | "blue" | "violet" | "amber" | "rose";
export type Density = "comfortable" | "compact";
export type SettingsSection = "general" | "phones" | "appearance" | "about";

export interface Prefs {
  theme: ThemePref;
  accent: Accent;
  density: Density;
  reduceMotion: boolean;
  /** Ask before forgetting a paired phone. */
  confirmForget: boolean;
  /** Show a toast when the phone sends a file. */
  fileToasts: boolean;
  /** Show a toast when the phone connects or drops. */
  connectionToasts: boolean;
  /** Show a toast when a text arrives outside the open conversation. */
  messageToasts: boolean;
  /** Open the phone app and activate a notification when its macOS banner is clicked. */
  openNotificationOnClick: boolean;
}

const DEFAULT_PREFS: Prefs = {
  theme: "system",
  accent: "green",
  density: "comfortable",
  reduceMotion: false,
  confirmForget: true,
  fileToasts: true,
  connectionToasts: true,
  messageToasts: true,
  openNotificationOnClick: true,
};

interface UiStore {
  tab: Tab;
  settingsOpen: boolean;
  settingsSection: SettingsSection;
  paletteOpen: boolean;
  activityOpen: boolean;
  prefs: Prefs;
  setTab: (tab: Tab) => void;
  openSettings: (section?: SettingsSection) => void;
  closeSettings: () => void;
  setSettingsSection: (section: SettingsSection) => void;
  setPalette: (open: boolean) => void;
  setActivity: (open: boolean) => void;
  setPref: <K extends keyof Prefs>(key: K, value: Prefs[K]) => void;
}

export const useUi = create<UiStore>()(
  persist(
    (set) => ({
      tab: "notifications",
      settingsOpen: false,
      settingsSection: "general",
      paletteOpen: false,
      activityOpen: false,
      prefs: DEFAULT_PREFS,
      setTab: (tab) => set({ tab }),
      openSettings: (section) => set((s) => ({ settingsOpen: true, paletteOpen: false, settingsSection: section ?? s.settingsSection })),
      closeSettings: () => set({ settingsOpen: false }),
      setSettingsSection: (settingsSection) => set({ settingsSection }),
      setPalette: (paletteOpen) => set({ paletteOpen }),
      setActivity: (activityOpen) => set({ activityOpen }),
      setPref: (key, value) => set((s) => ({ prefs: { ...s.prefs, [key]: value } })),
    }),
    {
      name: "smartconnect.ui",
      version: 1,
      partialize: (s) => ({ tab: s.tab, prefs: s.prefs }),
      merge: (persisted, current) => {
        const saved = (persisted ?? {}) as Partial<UiStore>;
        return { ...current, tab: saved.tab ?? current.tab, prefs: { ...DEFAULT_PREFS, ...saved.prefs } };
      },
    },
  ),
);
