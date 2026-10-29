import { motion } from "motion/react";
import { Check, Monitor, Moon, Sun } from "lucide-react";
import { Segmented } from "@/components/ui/Segmented";
import { Switch } from "@/components/ui/Switch";
import { cn } from "@/lib/cn";
import { useUi, type Accent } from "@/store/ui";
import { SettingGroup, SettingRow } from "./SettingRow";

const ACCENTS: { value: Accent; label: string; color: string }[] = [
  { value: "green", label: "Green", color: "#22b867" },
  { value: "blue", label: "Blue", color: "#3b82f6" },
  { value: "violet", label: "Violet", color: "#8b5cf6" },
  { value: "amber", label: "Amber", color: "#f5a524" },
  { value: "rose", label: "Rose", color: "#f43f5e" },
];

export function AppearanceSection() {
  const prefs = useUi((s) => s.prefs);
  const setPref = useUi((s) => s.setPref);

  return (
    <>
      <SettingGroup title="Theme">
        <SettingRow label="Appearance" description="Follow macOS, or pick one.">
          <Segmented
            id="theme"
            label="Appearance"
            value={prefs.theme}
            onChange={(v) => setPref("theme", v)}
            options={[
              { value: "system", label: "Auto", icon: <Monitor size={12} /> },
              { value: "light", label: "Light", icon: <Sun size={12} /> },
              { value: "dark", label: "Dark", icon: <Moon size={12} /> },
            ]}
          />
        </SettingRow>
        <SettingRow label="Accent color">
          <div role="radiogroup" aria-label="Accent color" className="flex gap-2">
            {ACCENTS.map((accent) => {
              const selected = prefs.accent === accent.value;
              return (
                <button
                  key={accent.value}
                  type="button"
                  role="radio"
                  aria-checked={selected}
                  aria-label={accent.label}
                  title={accent.label}
                  onClick={() => setPref("accent", accent.value)}
                  className={cn(
                    "relative grid size-6 place-items-center rounded-full transition-transform duration-200 ease-spring hover:scale-110 active:scale-95",
                  )}
                  style={{ background: accent.color }}
                >
                  {selected && (
                    <>
                      <motion.span
                        layoutId="accent-ring"
                        className="absolute -inset-[3px] rounded-full"
                        style={{ boxShadow: `0 0 0 2px ${accent.color}` }}
                        transition={{ type: "spring", bounce: 0.25, duration: 0.35 }}
                      />
                      <Check size={12} strokeWidth={3} className="text-white" />
                    </>
                  )}
                </button>
              );
            })}
          </div>
        </SettingRow>
      </SettingGroup>

      <SettingGroup title="Layout & motion">
        <SettingRow label="Density" description="Compact fits more on smaller screens.">
          <Segmented
            id="density"
            label="Density"
            value={prefs.density}
            onChange={(v) => setPref("density", v)}
            options={[
              { value: "comfortable", label: "Comfortable" },
              { value: "compact", label: "Compact" },
            ]}
          />
        </SettingRow>
        <SettingRow label="Reduce motion" description="Turns off animations. Also follows the macOS accessibility setting.">
          <Switch label="Reduce motion" checked={prefs.reduceMotion} onChange={(v) => setPref("reduceMotion", v)} />
        </SettingRow>
      </SettingGroup>
    </>
  );
}
