import { Activity, Plus, Search, Settings2 } from "lucide-react";
import { Button } from "@/components/ui/Button";
import { IconButton } from "@/components/ui/IconButton";
import { Kbd } from "@/components/ui/Kbd";
import { beginPairing } from "@/engine/actions";
import { usePaired } from "@/store/engine";
import { useUi } from "@/store/ui";
import { Brand } from "./Brand";
import { DevicePill } from "./DevicePill";

/**
 * Unified title bar. The native title bar is an overlay (tauri.conf.json), so
 * this leaves room for the traffic lights and marks empty space as draggable.
 */
export function TitleBar() {
  const paired = usePaired();
  const activityOpen = useUi((s) => s.activityOpen);
  const setActivity = useUi((s) => s.setActivity);
  const setPalette = useUi((s) => s.setPalette);
  const openSettings = useUi((s) => s.openSettings);

  return (
    <header
      data-tauri-drag-region
      className="relative z-30 flex h-[52px] shrink-0 items-center gap-3 border-b border-line bg-sidebar/85 pl-[86px] pr-3 backdrop-blur-xl"
    >
      <Brand />
      {paired && (
        <>
          <span data-tauri-drag-region className="h-4 w-px bg-line-strong" />
          <DevicePill />
        </>
      )}

      <div data-tauri-drag-region className="h-full flex-1" />

      <button
        type="button"
        onClick={() => setPalette(true)}
        className="group flex h-8 w-[min(260px,26vw)] items-center gap-2 rounded-lg border border-line bg-surface/60 px-2.5 text-xs text-fg-subtle transition hover:border-line-strong hover:bg-surface hover:text-fg-muted"
      >
        <Search size={14} />
        <span className="flex-1 truncate text-left">Search or run a command…</span>
        <Kbd>⌘K</Kbd>
      </button>

      {paired && (
        <Button variant="ghost" size="md" icon={<Plus size={15} />} onClick={() => void beginPairing()}>
          Pair phone
        </Button>
      )}
      <IconButton
        label="Activity"
        shortcut="⌘L"
        icon={<Activity size={16} />}
        active={activityOpen}
        onClick={() => setActivity(!activityOpen)}
      />
      <IconButton label="Settings" shortcut="⌘," icon={<Settings2 size={16} />} onClick={() => openSettings()} />
    </header>
  );
}
