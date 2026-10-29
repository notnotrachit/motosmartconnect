import { useEffect, useState } from "react";
import { getVersion } from "@tauri-apps/api/app";
import { BrandMark } from "@/components/layout/Brand";
import { Kbd } from "@/components/ui/Kbd";
import { SettingGroup } from "./SettingRow";

const SHORTCUTS: [string, string][] = [
  ["Command menu", "⌘K"],
  ["Settings", "⌘,"],
  ["Activity log", "⌘L"],
  ["Notifications · Messages · Apps · Files · Control", "⌘1–5"],
  ["New message", "⌘N"],
  ["Close a sheet or menu", "esc"],
];

export function AboutSection() {
  const [version, setVersion] = useState("");
  useEffect(() => {
    getVersion().then(setVersion).catch(() => {});
  }, []);

  return (
    <>
      <div className="mb-7 flex items-center gap-4">
        <BrandMark size={52} className="rounded-2xl" />
        <div>
          <p className="font-display text-base font-semibold text-fg">Smart Connect for Mac</p>
          <p className="text-xs text-fg-muted">{version ? `Version ${version}` : "\u00a0"}</p>
          <p className="mt-1 max-w-[46ch] text-xs leading-relaxed text-fg-subtle">
            Unofficial macOS host for the Motorola Smart Connect phone app. Not affiliated with Motorola or Lenovo.
          </p>
        </div>
      </div>

      <SettingGroup title="Keyboard shortcuts" description="Shortcuts pause while the phone screen or pad has focus, since keys go to the phone.">
        {SHORTCUTS.map(([label, keys]) => (
          <div key={label} className="flex items-center justify-between px-4 py-2.5 text-[0.8125rem] text-fg">
            {label}
            <Kbd className="h-5 px-1.5 text-[11px]">{keys}</Kbd>
          </div>
        ))}
      </SettingGroup>
    </>
  );
}
