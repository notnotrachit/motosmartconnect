import { useRef, type KeyboardEvent } from "react";
import { motion } from "motion/react";
import { Bell, FolderOpen, LayoutGrid, MessageSquare, MousePointer2, type LucideIcon } from "lucide-react";
import { CountBadge } from "@/components/ui/Feedback";
import { Kbd } from "@/components/ui/Kbd";
import { refreshIcons } from "@/engine/sync";
import { cn } from "@/lib/cn";
import { useApps, useEngine, useMessages, useNotifications } from "@/store/engine";
import { TABS, useUi, type Tab } from "@/store/ui";

export const TAB_META: Record<Tab, { label: string; icon: LucideIcon }> = {
  notifications: { label: "Notifications", icon: Bell },
  messages: { label: "Messages", icon: MessageSquare },
  apps: { label: "Apps", icon: LayoutGrid },
  files: { label: "Files", icon: FolderOpen },
  control: { label: "Control", icon: MousePointer2 },
};

export const tabId = (tab: Tab) => `tab-${tab}`;
export const panelId = (tab: Tab) => `panel-${tab}`;

export function selectTab(tab: Tab) {
  useUi.getState().setTab(tab);
  if (tab === "apps") void refreshIcons();
}

/** Underlined tab strip with a sliding indicator, counts, and arrow-key navigation. */
export function Tabs() {
  const tab = useUi((s) => s.tab);
  const notifications = useNotifications().length;
  const unreadMessages = useMessages().unread;
  const apps = useApps().length;
  const crossActive = useEngine((s) => s.snapshot?.cross?.active ?? false);
  const refs = useRef<Partial<Record<Tab, HTMLButtonElement | null>>>({});

  const onKeyDown = (event: KeyboardEvent) => {
    const index = TABS.indexOf(tab);
    const next =
      event.key === "ArrowRight" ? TABS[(index + 1) % TABS.length]
      : event.key === "ArrowLeft" ? TABS[(index - 1 + TABS.length) % TABS.length]
      : event.key === "Home" ? TABS[0]
      : event.key === "End" ? TABS[TABS.length - 1]
      : null;
    if (!next) return;
    event.preventDefault();
    selectTab(next);
    refs.current[next]?.focus();
  };

  return (
    <div role="tablist" aria-label="Phone features" onKeyDown={onKeyDown} className="flex items-end gap-1 overflow-x-auto border-b border-line px-5">
      {TABS.map((name, index) => {
        const { label, icon: Icon } = TAB_META[name];
        const selected = name === tab;
        return (
          <button
            key={name}
            ref={(el) => {
              refs.current[name] = el;
            }}
            id={tabId(name)}
            type="button"
            role="tab"
            aria-selected={selected}
            aria-controls={panelId(name)}
            tabIndex={selected ? 0 : -1}
            onClick={() => selectTab(name)}
            className={cn(
              "group relative flex h-11 shrink-0 items-center gap-2 rounded-t-lg px-3 text-[0.8125rem] font-medium transition-colors duration-150",
              selected ? "text-fg" : "text-fg-muted hover:text-fg",
            )}
          >
            <Icon size={15} className={cn("transition-colors", selected ? "text-accent" : "text-fg-subtle group-hover:text-fg-muted")} />
            {label}
            {name === "notifications" && <CountBadge count={notifications} />}
            {name === "messages" && <CountBadge count={unreadMessages} />}
            {name === "apps" && apps > 0 && <CountBadge count={apps} tone="muted" />}
            {name === "control" && crossActive && <span className="size-1.5 rounded-full bg-success" aria-label="active" />}
            <Kbd className="ml-0.5 opacity-0 transition-opacity group-hover:opacity-100">⌘{index + 1}</Kbd>
            {selected && (
              <motion.span
                layoutId="tab-indicator"
                className="absolute inset-x-2 -bottom-px h-[2px] rounded-full bg-accent"
                transition={{ type: "spring", bounce: 0.2, duration: 0.4 }}
              />
            )}
          </button>
        );
      })}
    </div>
  );
}
