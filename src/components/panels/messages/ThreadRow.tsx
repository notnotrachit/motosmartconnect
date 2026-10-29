import { memo } from "react";
import { motion } from "motion/react";
import { PenLine } from "lucide-react";
import { Avatar } from "@/components/ui/Avatar";
import { Spinner } from "@/components/ui/Spinner";
import { cn } from "@/lib/cn";
import type { ThreadView } from "@/lib/types";
import { isBareNumber, listTime } from "./format";
import { Highlight } from "./Highlight";

interface ThreadRowProps {
  thread: ThreadView;
  selected: boolean;
  opening: boolean;
  draft: string;
  query: string;
  now: number;
  onOpen: (key: string) => void;
}

export const ThreadRow = memo(function ThreadRow({ thread, selected, opening, draft, query, now, onOpen }: ThreadRowProps) {
  const unread = thread.unread > 0;
  return (
    <motion.li layout="position" initial={{ opacity: 0, y: 4 }} animate={{ opacity: 1, y: 0 }} transition={{ duration: 0.2 }}>
      <button
        type="button"
        data-thread={thread.key}
        onClick={() => onOpen(thread.key)}
        aria-current={selected ? "true" : undefined}
        aria-label={`${thread.title}${unread ? `, ${thread.unread} unread` : ""}`}
        className={cn(
          "group relative flex w-full items-center gap-3 rounded-xl px-2.5 py-2.5 text-left outline-none transition-colors duration-150",
          "focus-visible:ring-2 focus-visible:ring-accent/50",
          selected ? "bg-accent/12 dark:bg-accent/15" : "hover:bg-surface-2",
        )}
      >
        {/* Unread marker in the gutter, like Mail. */}
        <span
          aria-hidden="true"
          className={cn(
            "absolute left-0.5 top-1/2 size-1.5 -translate-y-1/2 rounded-full bg-accent transition-transform duration-300 ease-spring",
            unread ? "scale-100" : "scale-0",
          )}
        />
        <span className="relative">
          <Avatar name={thread.title} anonymous={isBareNumber(thread.title)} size={38} />
          {opening && (
            <span className="absolute inset-0 grid place-items-center rounded-full bg-black/45 text-white">
              <Spinner size={14} />
            </span>
          )}
        </span>
        <span className="min-w-0 flex-1">
          <span className="flex items-baseline gap-2">
            <span className={cn("truncate text-[0.8125rem] text-fg", unread ? "font-semibold" : "font-medium")}>
              <Highlight text={thread.title} query={query} />
            </span>
            <span className={cn("ml-auto shrink-0 text-[10.5px] tabular-nums", unread ? "font-semibold text-accent" : "text-fg-subtle")}>
              {listTime(thread.date, now)}
            </span>
          </span>
          <span className="mt-0.5 flex items-center gap-2">
            <span className={cn("line-clamp-1 flex-1 text-xs leading-snug", unread ? "text-fg" : "text-fg-muted")}>
              {draft && !selected ? (
                <>
                  <span className="inline-flex items-center gap-1 font-medium text-warning">
                    <PenLine size={10} /> Draft:
                  </span>{" "}
                  {draft}
                </>
              ) : (
                <Highlight text={thread.preview} query={query} />
              )}
            </span>
            {unread && (
              <span className="grid h-[18px] min-w-[18px] shrink-0 place-items-center rounded-full bg-accent px-1 text-[10.5px] font-semibold tabular-nums text-on-accent">
                {thread.unread > 99 ? "99+" : thread.unread}
              </span>
            )}
          </span>
        </span>
      </button>
    </motion.li>
  );
});
