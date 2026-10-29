import { AnimatePresence, motion } from "motion/react";
import { Info, Palette, Smartphone, SlidersHorizontal, X, type LucideIcon } from "lucide-react";
import { Dialog } from "@/components/ui/Dialog";
import { IconButton } from "@/components/ui/IconButton";
import { cn } from "@/lib/cn";
import { usePhones } from "@/store/engine";
import { useUi, type SettingsSection } from "@/store/ui";
import { AboutSection } from "./AboutSection";
import { AppearanceSection } from "./AppearanceSection";
import { GeneralSection } from "./GeneralSection";
import { PhonesSection } from "./PhonesSection";

const SECTIONS: { id: SettingsSection; label: string; icon: LucideIcon; render: () => React.JSX.Element }[] = [
  { id: "general", label: "General", icon: SlidersHorizontal, render: GeneralSection },
  { id: "phones", label: "Phones", icon: Smartphone, render: PhonesSection },
  { id: "appearance", label: "Appearance", icon: Palette, render: AppearanceSection },
  { id: "about", label: "About", icon: Info, render: AboutSection },
];

export function SettingsDialog() {
  const open = useUi((s) => s.settingsOpen);
  const section = useUi((s) => s.settingsSection);
  const setSection = useUi((s) => s.setSettingsSection);
  const close = useUi((s) => s.closeSettings);
  const phones = usePhones().length;
  const current = SECTIONS.find((s) => s.id === section) ?? SECTIONS[0];
  const Content = current.render;

  return (
    <Dialog open={open} onClose={close} size="xl" hideHeader className="h-[min(640px,86vh)]">
      <div className="flex min-h-0 flex-1">
        <nav aria-label="Settings sections" className="flex w-52 shrink-0 flex-col gap-0.5 border-r border-line bg-sidebar/60 p-3">
          <p className="px-2.5 pb-2 pt-1 font-display text-[0.95rem] font-semibold tracking-tight text-fg">Settings</p>
          {SECTIONS.map(({ id, label, icon: Icon }) => {
            const selected = id === section;
            return (
              <button
                key={id}
                type="button"
                onClick={() => setSection(id)}
                aria-current={selected ? "page" : undefined}
                className={cn(
                  "relative flex h-8 items-center gap-2.5 rounded-lg px-2.5 text-[0.8125rem] transition-colors",
                  selected ? "font-medium text-fg" : "text-fg-muted hover:bg-surface-2 hover:text-fg",
                )}
              >
                {selected && (
                  <motion.span
                    layoutId="settings-nav"
                    className="absolute inset-0 rounded-lg bg-surface shadow-card ring-1 ring-line dark:bg-surface-3"
                    transition={{ type: "spring", bounce: 0.18, duration: 0.35 }}
                  />
                )}
                <Icon size={15} className={cn("relative", selected && "text-accent")} />
                <span className="relative flex-1 text-left">{label}</span>
                {id === "phones" && phones > 0 && <span className="relative text-[11px] text-fg-subtle">{phones}</span>}
              </button>
            );
          })}
        </nav>

        <div className="flex min-w-0 flex-1 flex-col">
          <header className="flex h-14 shrink-0 items-center justify-between border-b border-line px-6">
            <h2 className="font-display text-[0.95rem] font-semibold tracking-tight text-fg">{current.label}</h2>
            <IconButton label="Close" shortcut="esc" icon={<X size={16} />} size="sm" onClick={close} />
          </header>
          <div className="min-h-0 flex-1 overflow-y-auto px-6 py-5">
            <AnimatePresence mode="wait" initial={false}>
              <motion.div
                key={section}
                initial={{ opacity: 0, y: 6 }}
                animate={{ opacity: 1, y: 0 }}
                exit={{ opacity: 0, y: -4 }}
                transition={{ duration: 0.16 }}
              >
                <Content />
              </motion.div>
            </AnimatePresence>
          </div>
        </div>
      </div>
    </Dialog>
  );
}
