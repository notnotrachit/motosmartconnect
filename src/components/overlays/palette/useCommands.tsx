import { useMemo, type ReactNode } from "react";
import {
  Activity,
  Camera,
  Monitor,
  MousePointer2,
  Moon,
  Palette,
  Plus,
  RefreshCw,
  ScreenShare,
  Settings2,
  Smartphone,
  Square,
  SquarePen,
  Sun,
} from "lucide-react";
import { TAB_META, selectTab } from "@/components/panels/Tabs";
import { AppIcon } from "@/components/ui/AppIcon";
import {
  beginPairing,
  crossStart,
  crossStop,
  openApp,
  refreshApps,
  refreshCode,
  startDesktop,
  startMirror,
  startWebcam,
  stopStream,
} from "@/engine/actions";
import { useResolvedTheme } from "@/hooks/useApplyPrefs";
import { Avatar } from "@/components/ui/Avatar";
import { isBareNumber } from "@/components/panels/messages/format";
import { openThread } from "@/engine/actions";
import { useApps, useConnectedPhone, useEngine, useIcons, useMessages, usePaired } from "@/store/engine";
import { composeMessage, useMessageUi } from "@/store/messages";
import { TABS, useUi } from "@/store/ui";

export interface Command {
  id: string;
  title: string;
  group: "Phone" | "Go to" | "Messages" | "Apps" | "App";
  icon: ReactNode;
  keywords?: string;
  shortcut?: string;
  disabled?: boolean;
  run: () => unknown;
}

/** Everything the command menu can do, given the current state. */
export function useCommands(): Command[] {
  const paired = usePaired();
  const connected = Boolean(useConnectedPhone());
  const crossActive = useEngine((s) => s.snapshot?.cross?.active ?? false);
  const apps = useApps();
  const sms = useMessages();
  const icons = useIcons();
  const theme = useResolvedTheme();
  const ui = useUi.getState;

  return useMemo(() => {
    const list: Command[] = [];
    const offline = !connected;

    if (paired) {
      list.push(
        { id: "mirror", group: "Phone", title: "Mirror phone screen", icon: <ScreenShare size={15} />, keywords: "screen cast share", disabled: offline, run: startMirror },
        { id: "desktop", group: "Phone", title: "Open phone desktop", icon: <Monitor size={15} />, keywords: "ready for desktop window", disabled: offline, run: startDesktop },
        { id: "camera", group: "Phone", title: "Use phone as camera", icon: <Camera size={15} />, keywords: "webcam video obs", disabled: offline, run: startWebcam },
        { id: "stop", group: "Phone", title: "Stop screen", icon: <Square size={14} />, keywords: "close end", run: stopStream },
        {
          id: "cross",
          group: "Phone",
          title: crossActive ? "Stop cross control" : "Start cross control",
          icon: <MousePointer2 size={15} />,
          keywords: "mouse keyboard pointer",
          disabled: offline && !crossActive,
          run: () => {
            selectTab("control");
            return crossActive ? crossStop() : crossStart();
          },
        },
        { id: "refresh-apps", group: "Phone", title: "Refresh app list", icon: <RefreshCw size={15} />, disabled: offline, run: refreshApps },
        { id: "pair", group: "Phone", title: "Pair another phone", icon: <Plus size={15} />, keywords: "qr code add", run: beginPairing },
        {
          id: "new-message",
          group: "Phone",
          title: "New message",
          icon: <SquarePen size={15} />,
          shortcut: "⌘N",
          keywords: "sms text compose send",
          disabled: offline || sms.phase !== "ready",
          run: composeMessage,
        },
      );
      TABS.forEach((tab, index) => {
        const meta = TAB_META[tab];
        const Icon = meta.icon;
        list.push({ id: `tab-${tab}`, group: "Go to", title: meta.label, icon: <Icon size={15} />, shortcut: `⌘${index + 1}`, run: () => selectTab(tab) });
      });
    } else {
      list.push({ id: "new-code", group: "Phone", title: "New pairing code", icon: <RefreshCw size={15} />, keywords: "qr refresh", run: refreshCode });
    }

    list.push(
      { id: "settings", group: "App", title: "Settings", icon: <Settings2 size={15} />, shortcut: "⌘,", keywords: "preferences", run: () => ui().openSettings() },
      { id: "phones", group: "App", title: "Manage phones", icon: <Smartphone size={15} />, keywords: "forget paired", run: () => ui().openSettings("phones") },
      { id: "appearance", group: "App", title: "Appearance", icon: <Palette size={15} />, keywords: "theme accent color density", run: () => ui().openSettings("appearance") },
      {
        id: "theme",
        group: "App",
        title: theme === "dark" ? "Switch to light theme" : "Switch to dark theme",
        icon: theme === "dark" ? <Sun size={15} /> : <Moon size={15} />,
        keywords: "appearance mode",
        run: () => ui().setPref("theme", theme === "dark" ? "light" : "dark"),
      },
      { id: "activity", group: "App", title: "Show activity log", icon: <Activity size={15} />, shortcut: "⌘L", keywords: "logs debug", run: () => ui().setActivity(true) },
    );

    if (paired) {
      for (const thread of sms.threads) {
        list.push({
          id: `thread-${thread.key}`,
          group: "Messages",
          title: `Message ${thread.title}`,
          keywords: `${thread.address} sms text`,
          icon: <Avatar name={thread.title} anonymous={isBareNumber(thread.title)} size={18} />,
          run: () => {
            selectTab("messages");
            useMessageUi.getState().stopCompose();
            return openThread(thread.key);
          },
        });
      }
    }

    if (paired && connected) {
      for (const app of apps) {
        const name = app.name || app.pkg;
        list.push({
          id: `app-${app.pkg}-${app.userId}`,
          group: "Apps",
          title: `Open ${name}`,
          keywords: app.pkg,
          icon: <AppIcon src={(app.iconName && icons[app.iconName]) || ""} name={name} size={18} />,
          run: () => openApp(app),
        });
      }
    }
    return list;
  }, [paired, connected, crossActive, apps, sms, icons, theme, ui]);
}
