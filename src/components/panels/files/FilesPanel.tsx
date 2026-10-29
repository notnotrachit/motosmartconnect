import { AnimatePresence, motion } from "motion/react";
import { ExternalLink, Inbox } from "lucide-react";
import { Button } from "@/components/ui/Button";
import { reveal } from "@/engine/actions";
import { useConnectedPhone, useFiles } from "@/store/engine";
import { PanelHeader } from "../PanelHeader";
import { FileGlyph } from "./FileGlyph";
import { PhoneBrowser } from "./PhoneBrowser";

export function FilesPanel() {
  const files = useFiles();
  const connected = Boolean(useConnectedPhone());

  return (
    <>
      <PanelHeader title="Files" subtitle="Saved to Downloads › Smart Connect" />
      <div className="min-h-0 flex-1 space-y-7 overflow-y-auto px-6 pb-6">
        <section aria-labelledby="phone-storage">
          <h3 id="phone-storage" className="mb-2.5 text-[11px] font-semibold uppercase tracking-wider text-fg-subtle">
            Phone storage
          </h3>
          <PhoneBrowser connected={connected} />
        </section>

        <section aria-labelledby="received">
          <h3 id="received" className="mb-2.5 flex items-center gap-2 text-[11px] font-semibold uppercase tracking-wider text-fg-subtle">
            Received from phone
            {files.length > 0 && <span className="rounded-full bg-surface-3 px-1.5 py-px text-[10px] text-fg-muted">{files.length}</span>}
          </h3>
          {!files.length ? (
            <div className="flex items-center gap-3 rounded-2xl border border-line bg-surface/60 px-4 py-4 text-[0.8125rem] text-fg-muted">
              <Inbox size={18} className="text-fg-subtle" />
              Files and images you share from the phone land here.
            </div>
          ) : (
            <ul className="overflow-hidden rounded-2xl border border-line bg-surface shadow-card">
              <AnimatePresence initial={false}>
                {files.map((file) => (
                  <motion.li
                    key={file.path}
                    layout
                    initial={{ opacity: 0, height: 0 }}
                    animate={{ opacity: 1, height: "auto" }}
                    exit={{ opacity: 0, height: 0 }}
                    className="border-b border-line last:border-b-0"
                  >
                    <div className="group flex items-center gap-3 px-4 py-2 transition-colors hover:bg-surface-2">
                      <FileGlyph name={file.name} />
                      <span className="min-w-0 flex-1 truncate text-[0.8125rem] text-fg" title={file.path}>
                        {file.name}
                      </span>
                      <Button
                        size="sm"
                        variant="ghost"
                        icon={<ExternalLink size={12} />}
                        className="opacity-70 group-hover:opacity-100"
                        onClick={() => void reveal(file.path)}
                      >
                        Show in Finder
                      </Button>
                    </div>
                  </motion.li>
                ))}
              </AnimatePresence>
            </ul>
          )}
        </section>
      </div>
    </>
  );
}
