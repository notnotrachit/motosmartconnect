import { IconButton } from "@/components/ui/IconButton";
import { pressAndroidKey } from "@/engine/actions";
import { ANDROID_KEYS } from "@/lib/keymap";
import { BackGlyph, HomeGlyph, RecentGlyph } from "./glyphs";

/** Navigation pill under the phone. */
export function AndroidKeys({ disabled }: { disabled: boolean }) {
  const keys = [
    { label: "Back", code: ANDROID_KEYS.back, glyph: <BackGlyph /> },
    { label: "Home", code: ANDROID_KEYS.home, glyph: <HomeGlyph /> },
    { label: "Recent apps", code: ANDROID_KEYS.recent, glyph: <RecentGlyph /> },
  ];
  return (
    <div
      role="toolbar"
      aria-label="Phone navigation"
      className="flex items-center gap-6 rounded-full border border-line bg-surface/80 px-4 py-1 shadow-card backdrop-blur"
    >
      {keys.map((key) => (
        <IconButton
          key={key.label}
          label={key.label}
          tooltipSide="top"
          icon={key.glyph}
          size="md"
          className="rounded-full"
          disabled={disabled}
          onClick={() => void pressAndroidKey(key.code)}
        />
      ))}
    </div>
  );
}
