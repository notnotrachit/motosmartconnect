import { useEffect } from "react";
import { TABS, useUi } from "@/store/ui";
import { usePaired } from "@/store/engine";
import { refreshIcons } from "@/engine/sync";
import { composeMessage } from "@/store/messages";

/**
 * App-wide keyboard shortcuts:
 *   ⌘K command menu · ⌘, settings · ⌘1–5 tabs · ⌘L activity log
 * Ignored while the phone screen or pad has focus, because those keys go to the phone.
 */
export function useHotkeys() {
  const paired = usePaired();

  useEffect(() => {
    const onKeyDown = (event: KeyboardEvent) => {
      if (!event.metaKey || event.altKey || event.ctrlKey) return;
      if ((event.target as Element | null)?.closest?.("[data-forward-keys]")) return;
      const ui = useUi.getState();
      const key = event.key.toLowerCase();

      if (key === "k") {
        event.preventDefault();
        ui.setPalette(!ui.paletteOpen);
      } else if (key === ",") {
        event.preventDefault();
        ui.openSettings();
      } else if (key === "l") {
        event.preventDefault();
        ui.setActivity(!ui.activityOpen);
      } else if (paired && key === "n") {
        event.preventDefault();
        composeMessage();
      } else if (paired && /^[1-5]$/.test(key)) {
        event.preventDefault();
        const tab = TABS[Number(key) - 1];
        ui.setTab(tab);
        if (tab === "apps") void refreshIcons();
      }
    };
    window.addEventListener("keydown", onKeyDown);
    return () => window.removeEventListener("keydown", onKeyDown);
  }, [paired]);
}
