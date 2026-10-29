import { AnimatePresence, motion } from "motion/react";
import { PhoneStage } from "@/components/stage/PhoneStage";
import { useUi, type Tab } from "@/store/ui";
import { AppsPanel } from "./apps/AppsPanel";
import { ControlPanel } from "./control/ControlPanel";
import { FilesPanel } from "./files/FilesPanel";
import { MessagesPanel } from "./messages/MessagesPanel";
import { NotificationsPanel } from "./notifications/NotificationsPanel";
import { panelId, tabId, Tabs } from "./Tabs";

const PANELS: Record<Tab, () => React.JSX.Element> = {
  notifications: NotificationsPanel,
  messages: MessagesPanel,
  apps: AppsPanel,
  files: FilesPanel,
  control: ControlPanel,
};

/** Paired layout: the phone on the left, feature panels on the right. */
export function Workspace() {
  const tab = useUi((s) => s.tab);
  const Panel = PANELS[tab];

  return (
    <motion.div
      initial={{ opacity: 0 }}
      animate={{ opacity: 1 }}
      exit={{ opacity: 0 }}
      transition={{ duration: 0.25 }}
      className="absolute inset-0 flex"
    >
      <PhoneStage />
      <section className="flex min-w-0 flex-1 flex-col bg-canvas">
        <Tabs />
        <div className="relative min-h-0 flex-1">
          <AnimatePresence mode="wait" initial={false}>
            <motion.div
              key={tab}
              id={panelId(tab)}
              role="tabpanel"
              aria-labelledby={tabId(tab)}
              initial={{ opacity: 0, y: 8 }}
              animate={{ opacity: 1, y: 0 }}
              exit={{ opacity: 0, y: -4 }}
              transition={{ duration: 0.18, ease: [0.16, 1, 0.3, 1] }}
              className="absolute inset-0 flex flex-col"
            >
              <Panel />
            </motion.div>
          </AnimatePresence>
        </div>
      </section>
    </motion.div>
  );
}
