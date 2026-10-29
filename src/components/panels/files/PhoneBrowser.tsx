import { Fragment, useDeferredValue, useMemo, useState } from "react";
import { AnimatePresence, motion } from "motion/react";
import { ArrowUp, ChevronRight, Download, FolderOpen, FolderX, HardDrive, RefreshCw, Search, Smartphone } from "lucide-react";
import { AsyncButton } from "@/components/ui/AsyncButton";
import { Button } from "@/components/ui/Button";
import { EmptyState, Skeleton } from "@/components/ui/Feedback";
import { IconButton } from "@/components/ui/IconButton";
import { Input } from "@/components/ui/Input";
import { savePhoneFile } from "@/engine/actions";
import { cn } from "@/lib/cn";
import { formatBytes } from "@/lib/format";
import { FileGlyph } from "./FileGlyph";
import { usePhoneBrowser } from "./usePhoneBrowser";

/** Browse the phone's storage and save files to this Mac. */
export function PhoneBrowser({ connected }: { connected: boolean }) {
  const browser = usePhoneBrowser();
  const { state } = browser;
  const [filter, setFilter] = useState("");
  const deferred = useDeferredValue(filter);
  const crumbs = state.path ? state.path.split("/") : [];

  const entries = useMemo(() => {
    const q = deferred.trim().toLowerCase();
    return q ? state.entries.filter((e) => e.name.toLowerCase().includes(q)) : state.entries;
  }, [state.entries, deferred]);

  const go = (path: string, direction: 1 | -1) => {
    setFilter("");
    void browser.open(path, direction);
  };

  if (!state.opened) {
    return (
      <div className="rounded-2xl border border-dashed border-line-strong bg-surface/50">
        <EmptyState
          icon={<HardDrive size={20} />}
          title="Browse phone storage"
          description="Open folders on your phone and save any file to this Mac."
        >
          <Button variant="primary" icon={<FolderOpen size={14} />} disabled={!connected} onClick={() => go("", 1)}>
            Open phone storage
          </Button>
        </EmptyState>
      </div>
    );
  }

  return (
    <div className="overflow-hidden rounded-2xl border border-line bg-surface shadow-card">
      {/* Toolbar */}
      <div className="flex items-center gap-2 border-b border-line px-2.5 py-2">
        <IconButton label="Up one folder" icon={<ArrowUp size={14} />} size="sm" disabled={!state.path} onClick={browser.up} />
        <nav aria-label="Folder path" className="flex min-w-0 flex-1 items-center gap-0.5 overflow-hidden text-xs">
          <button
            type="button"
            onClick={() => go("", -1)}
            className={cn(
              "flex shrink-0 items-center gap-1.5 rounded-md px-1.5 py-1 transition hover:bg-surface-2",
              crumbs.length ? "text-fg-muted" : "font-medium text-fg",
            )}
          >
            <Smartphone size={12} /> Phone
          </button>
          {crumbs.map((crumb, index) => {
            const last = index === crumbs.length - 1;
            return (
              <Fragment key={`${index}-${crumb}`}>
                <ChevronRight size={12} className="shrink-0 text-fg-subtle" />
                <button
                  type="button"
                  onClick={() => go(crumbs.slice(0, index + 1).join("/"), -1)}
                  disabled={last}
                  className={cn(
                    "truncate rounded-md px-1.5 py-1 transition",
                    last ? "font-medium text-fg" : "text-fg-muted hover:bg-surface-2 hover:text-fg",
                  )}
                >
                  {crumb}
                </button>
              </Fragment>
            );
          })}
        </nav>
        <Input
          size="sm"
          value={filter}
          onChange={(event) => setFilter(event.target.value)}
          onClear={() => setFilter("")}
          placeholder="Filter"
          aria-label="Filter this folder"
          leading={<Search size={12} />}
          wrapperClassName="w-36"
        />
        <IconButton
          label="Reload folder"
          size="sm"
          icon={<RefreshCw size={13} className={state.loading ? "animate-spin" : undefined} />}
          onClick={browser.reload}
        />
      </div>

      {/* Listing */}
      <div className="relative min-h-[200px] overflow-hidden">
        <AnimatePresence mode="popLayout" initial={false} custom={state.direction}>
          <motion.div
            key={state.path + (state.loading ? ":loading" : "")}
            custom={state.direction}
            variants={{
              enter: (dir: number) => ({ opacity: 0, x: dir * 24 }),
              center: { opacity: 1, x: 0 },
              exit: (dir: number) => ({ opacity: 0, x: dir * -24 }),
            }}
            initial="enter"
            animate="center"
            exit="exit"
            transition={{ duration: 0.2, ease: [0.16, 1, 0.3, 1] }}
          >
            {state.loading ? (
              <ul className="divide-y divide-line">
                {Array.from({ length: 6 }, (_, i) => (
                  <li key={i} className="flex items-center gap-3 px-4 py-2.5">
                    <Skeleton className="size-8 rounded-lg" />
                    <Skeleton className="h-3 flex-1" />
                    <Skeleton className="h-3 w-12" />
                  </li>
                ))}
              </ul>
            ) : state.error ? (
              <EmptyState tone="error" icon={<FolderX size={20} />} title="Couldn't open this folder" description={state.error}>
                <Button size="sm" onClick={browser.reload}>
                  Try again
                </Button>
              </EmptyState>
            ) : !entries.length ? (
              <EmptyState
                icon={<FolderOpen size={20} />}
                title={filter ? "No matches" : "This folder is empty"}
                description={filter ? `Nothing here matches “${filter}”.` : undefined}
              />
            ) : (
              <ul className="max-h-[52vh] divide-y divide-line overflow-y-auto">
                {entries.map((entry) => (
                  <li key={entry.name}>
                    <div
                      className={cn(
                        "group flex items-center gap-3 px-4 py-2 transition-colors hover:bg-surface-2",
                        entry.directory && "cursor-pointer",
                      )}
                      onClick={entry.directory ? () => go(browser.pathOf(entry.name), 1) : undefined}
                      onKeyDown={(event) => {
                        if (entry.directory && event.key === "Enter") go(browser.pathOf(entry.name), 1);
                      }}
                      role={entry.directory ? "button" : undefined}
                      tabIndex={entry.directory ? 0 : undefined}
                    >
                      <FileGlyph name={entry.name} directory={entry.directory} />
                      <span className="min-w-0 flex-1 truncate text-[0.8125rem] text-fg">{entry.name}</span>
                      {!entry.directory && entry.size > 0 && (
                        <span className="shrink-0 font-mono text-[11px] tabular-nums text-fg-subtle">{formatBytes(entry.size)}</span>
                      )}
                      {entry.directory ? (
                        <ChevronRight size={14} className="shrink-0 text-fg-subtle transition-transform group-hover:translate-x-0.5" />
                      ) : (
                        <AsyncButton
                          size="sm"
                          variant="ghost"
                          icon={<Download size={12} />}
                          className="opacity-70 group-hover:opacity-100"
                          onClick={() => savePhoneFile(browser.pathOf(entry.name))}
                        >
                          Save
                        </AsyncButton>
                      )}
                    </div>
                  </li>
                ))}
              </ul>
            )}
          </motion.div>
        </AnimatePresence>
      </div>
    </div>
  );
}
