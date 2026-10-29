import { useEffect, useMemo, useRef, useState, type KeyboardEvent } from "react";
import { motion } from "motion/react";
import { CornerDownLeft, Search } from "lucide-react";
import { Dialog } from "@/components/ui/Dialog";
import { Kbd } from "@/components/ui/Kbd";
import { cn } from "@/lib/cn";
import { fuzzyScore } from "@/lib/format";
import { useUi } from "@/store/ui";
import { useCommands, type Command } from "./useCommands";

const GROUP_ORDER: Command["group"][] = ["Phone", "Go to", "Messages", "Apps", "App"];
const MAX_APPS_WITHOUT_QUERY = 6;

/** ⌘K menu: fuzzy search over actions, panels and phone apps. */
export function CommandPalette() {
  const open = useUi((s) => s.paletteOpen);
  const setPalette = useUi((s) => s.setPalette);
  const close = () => setPalette(false);

  return (
    <Dialog open={open} onClose={close} size="lg" placement="top" hideHeader className="max-w-xl">
      <PaletteBody onClose={close} />
    </Dialog>
  );
}

function PaletteBody({ onClose }: { onClose: () => void }) {
  const commands = useCommands();
  const [query, setQuery] = useState("");
  const [active, setActive] = useState(0);
  const list = useRef<HTMLDivElement>(null);

  const results = useMemo(() => {
    const q = query.trim();
    if (!q) {
      let apps = 0;
      // Conversations only appear once you search for them.
      return commands.filter((c) => c.group !== "Messages" && (c.group !== "Apps" || apps++ < MAX_APPS_WITHOUT_QUERY));
    }
    return commands
      .map((c) => ({ c, score: fuzzyScore(q, `${c.title} ${c.keywords ?? ""}`) }))
      .filter((r) => r.score >= 0)
      .sort((a, b) => b.score - a.score)
      .slice(0, 40)
      .map((r) => r.c);
  }, [commands, query]);

  // Grouped for display; flat order for keyboard navigation.
  const grouped = useMemo(() => {
    if (query.trim()) return [{ group: "Results", items: results }];
    return GROUP_ORDER.map((group) => ({ group, items: results.filter((c) => c.group === group) })).filter((g) => g.items.length);
  }, [results, query]);
  const flat = useMemo(() => grouped.flatMap((g) => g.items), [grouped]);

  // Start on the first command that can run. Only on a new query, so snapshots don't reset the selection.
  useEffect(() => setActive(Math.max(0, flat.findIndex((c) => !c.disabled))), [query]); // eslint-disable-line react-hooks/exhaustive-deps
  useEffect(() => {
    list.current?.querySelector(`[data-index="${active}"]`)?.scrollIntoView({ block: "nearest" });
  }, [active]);

  const run = (command: Command | undefined) => {
    if (!command || command.disabled) return;
    onClose();
    // Let the menu start closing before the action opens anything else.
    requestAnimationFrame(() => void command.run());
  };

  const onKeyDown = (event: KeyboardEvent) => {
    if (event.key === "ArrowDown") {
      event.preventDefault();
      setActive((i) => Math.min(flat.length - 1, i + 1));
    } else if (event.key === "ArrowUp") {
      event.preventDefault();
      setActive((i) => Math.max(0, i - 1));
    } else if (event.key === "Enter") {
      event.preventDefault();
      run(flat[active]);
    }
  };

  let index = -1;
  return (
    <div onKeyDown={onKeyDown}>
      <div className="flex items-center gap-3 border-b border-line px-4">
        <Search size={16} className="text-fg-subtle" />
        <input
          data-autofocus
          value={query}
          onChange={(event) => setQuery(event.target.value)}
          placeholder="Search commands and apps…"
          aria-label="Search commands"
          role="combobox"
          aria-expanded="true"
          aria-controls="palette-list"
          aria-activedescendant={flat[active] ? `cmd-${flat[active].id}` : undefined}
          className="h-13 flex-1 bg-transparent py-4 text-[0.95rem] text-fg outline-none placeholder:text-fg-subtle focus-visible:outline-none"
        />
        <Kbd>esc</Kbd>
      </div>

      <div ref={list} id="palette-list" role="listbox" className="max-h-[min(420px,55vh)] overflow-y-auto p-2">
        {!flat.length && <p className="px-3 py-10 text-center text-[0.8125rem] text-fg-muted">No results for “{query}”</p>}
        {grouped.map(({ group, items }) => (
          <div key={group} className="mb-1 last:mb-0">
            <p className="px-3 pb-1 pt-2 text-[10.5px] font-semibold uppercase tracking-wider text-fg-subtle">{group}</p>
            {items.map((command) => {
              index += 1;
              const i = index;
              const selected = i === active;
              return (
                <button
                  key={command.id}
                  id={`cmd-${command.id}`}
                  type="button"
                  role="option"
                  aria-selected={selected}
                  aria-disabled={command.disabled || undefined}
                  data-index={i}
                  onMouseMove={() => setActive(i)}
                  onClick={() => run(command)}
                  className={cn(
                    "relative flex h-9 w-full items-center gap-3 rounded-lg px-3 text-left text-[0.8125rem]",
                    command.disabled ? "text-fg-subtle" : "text-fg",
                  )}
                >
                  {selected && (
                    <motion.span
                      layoutId="palette-active"
                      className="absolute inset-0 rounded-lg bg-surface-2 ring-1 ring-line"
                      transition={{ type: "spring", bounce: 0.15, duration: 0.25 }}
                    />
                  )}
                  <span className={cn("relative flex w-5 justify-center", selected ? "text-accent" : "text-fg-muted")}>{command.icon}</span>
                  <span className="relative flex-1 truncate">{command.title}</span>
                  {command.disabled && <span className="relative text-[11px] text-fg-subtle">Phone offline</span>}
                  {command.shortcut && <Kbd className="relative">{command.shortcut}</Kbd>}
                  {selected && !command.disabled && <CornerDownLeft size={13} className="relative text-fg-subtle" />}
                </button>
              );
            })}
          </div>
        ))}
      </div>

      <div className="flex items-center gap-4 border-t border-line bg-surface-2/50 px-4 py-2 text-[11px] text-fg-subtle">
        <span className="flex items-center gap-1.5">
          <Kbd>↑</Kbd>
          <Kbd>↓</Kbd> navigate
        </span>
        <span className="flex items-center gap-1.5">
          <Kbd>↵</Kbd> run
        </span>
        <span className="ml-auto flex items-center gap-1.5">
          <Kbd>⌘K</Kbd> toggle
        </span>
      </div>
    </div>
  );
}
