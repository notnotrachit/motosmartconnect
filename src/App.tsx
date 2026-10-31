import { useEffect } from "react";
import { listen } from "@tauri-apps/api/event";
import { AnimatePresence, MotionConfig } from "motion/react";
import { BootScreen } from "@/components/layout/BootScreen";
import { StatusBar } from "@/components/layout/StatusBar";
import { TitleBar } from "@/components/layout/TitleBar";
import { Onboarding } from "@/components/onboarding/Onboarding";
import { PairDialog } from "@/components/onboarding/PairDialog";
import { ActivityDrawer } from "@/components/overlays/ActivityDrawer";
import { ConfirmDialog } from "@/components/overlays/ConfirmDialog";
import { CommandPalette } from "@/components/overlays/palette/CommandPalette";
import { SettingsDialog } from "@/components/overlays/settings/SettingsDialog";
import { Toaster } from "@/components/overlays/Toaster";
import { Workspace } from "@/components/panels/Workspace";
import { openBannerNotice, startMirror } from "@/engine/actions";
import { startEngineSync } from "@/engine/sync";
import { useApplyPrefs } from "@/hooks/useApplyPrefs";
import { useHotkeys } from "@/hooks/useHotkeys";
import { usePaired, useSnapshot } from "@/store/engine";
import { TABS, useUi, type Tab } from "@/store/ui";

interface BannerClick {
  key: string;
  pkg: string;
}

export function App() {
  const { reducedMotion } = useApplyPrefs();
  useHotkeys();
  useEffect(() => {
    let cancelled = false;
    let unlisten: (() => void) | undefined;
    void listen<string>("tray:navigate", (event) => {
      if (event.payload === "mirror") void startMirror();
      else if (event.payload === "settings") useUi.getState().openSettings();
      else if (TABS.includes(event.payload as Tab)) useUi.getState().setTab(event.payload as Tab);
    }).then((stop) => {
      if (cancelled) stop();
      else unlisten = stop;
    });
    return () => {
      cancelled = true;
      unlisten?.();
    };
  }, []);
  useEffect(() => {
    let cancelled = false;
    let unlisten: (() => void) | undefined;
    void listen<BannerClick>("notification-banner-click", ({ payload }) => {
      if (!useUi.getState().prefs.openNotificationOnClick) return;
      void openBannerNotice(payload.key, payload.pkg);
    }).then((stop) => {
      if (cancelled) stop();
      else unlisten = stop;
    });
    return () => {
      cancelled = true;
      unlisten?.();
    };
  }, []);
  const ready = Boolean(useSnapshot());
  const paired = usePaired();

  useEffect(() => {
    startEngineSync();
  }, []);

  const view = !ready ? "boot" : paired ? "workspace" : "onboarding";

  return (
    <MotionConfig reducedMotion={reducedMotion ? "always" : "user"}>
      <div className="flex h-full flex-col">
        <TitleBar />
        <main className="relative min-h-0 flex-1 overflow-hidden">
          <AnimatePresence mode="wait" initial={false}>
            {view === "boot" && <BootScreen key="boot" />}
            {view === "onboarding" && <Onboarding key="onboarding" />}
            {view === "workspace" && <Workspace key="workspace" />}
          </AnimatePresence>
        </main>
        <StatusBar />
      </div>

      <PairDialog />
      <SettingsDialog />
      <CommandPalette />
      <ActivityDrawer />
      <ConfirmDialog />
      <Toaster />
    </MotionConfig>
  );
}
