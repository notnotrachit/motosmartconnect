import { memo, useState } from "react";
import { AnimatePresence, motion } from "motion/react";
import { ArrowUpRight, Reply } from "lucide-react";
import { AppIcon } from "@/components/ui/AppIcon";
import { AsyncButton } from "@/components/ui/AsyncButton";
import { Button } from "@/components/ui/Button";
import { Spinner } from "@/components/ui/Spinner";
import { actOnNotice, openNotice } from "@/engine/actions";
import { cn } from "@/lib/cn";
import type { NoticeView } from "@/lib/types";
import { ReplyComposer } from "./ReplyComposer";

interface NotificationCardProps {
  notice: NoticeView;
  icon: string;
}

export const NotificationCard = memo(function NotificationCard({ notice, icon }: NotificationCardProps) {
  const [replying, setReplying] = useState(false);
  const [opening, setOpening] = useState(false);
  const app = notice.app || "Phone";
  const actions = (notice.actions ?? []).filter((action) => !(action.name === "Reply" && notice.canReply));

  const open = async () => {
    if (opening) return;
    setOpening(true);
    await openNotice(notice);
    setOpening(false);
  };

  return (
    <motion.li
      layout="position"
      initial={{ opacity: 0, y: 10, scale: 0.98 }}
      animate={{ opacity: 1, y: 0, scale: 1 }}
      exit={{ opacity: 0, x: -24, transition: { duration: 0.16 } }}
      transition={{ type: "spring", duration: 0.4, bounce: 0.12 }}
    >
      <article
        tabIndex={0}
        onClick={(event) => {
          if ((event.target as Element).closest("button, input, form")) return;
          void open();
        }}
        onKeyDown={(event) => {
          if (event.key === "Enter" && event.target === event.currentTarget) void open();
        }}
        aria-label={`${app}: ${notice.title}. Press Enter to open on the phone.`}
        className={cn(
          "group relative flex cursor-pointer gap-3 rounded-2xl border border-line bg-surface p-3.5 shadow-card outline-none",
          "transition-[border-color,box-shadow,transform] duration-200 hover:-translate-y-px hover:border-line-strong hover:shadow-[0_8px_24px_-12px_rgb(0_0_0/0.25)]",
          "focus-visible:ring-2 focus-visible:ring-accent/50",
          replying && "border-accent/40 ring-2 ring-accent/15 hover:translate-y-0",
        )}
      >
        <div className="relative">
          <AppIcon src={icon} name={app} size={38} />
          {opening && (
            <span className="absolute inset-0 grid place-items-center rounded-[10px] bg-black/40 text-white">
              <Spinner size={14} />
            </span>
          )}
        </div>

        <div className="min-w-0 flex-1">
          <div className="flex items-center gap-2">
            <span className="truncate text-xs font-medium text-fg-muted">{app}</span>
            <span className="ml-auto flex items-center gap-0.5 text-[11px] font-medium text-fg-subtle opacity-0 transition-opacity group-hover:opacity-100">
              Open on phone <ArrowUpRight size={12} />
            </span>
          </div>
          {notice.title && <h3 className="mt-0.5 truncate text-[0.8125rem] font-semibold text-fg">{notice.title}</h3>}
          {notice.content && (
            <p className="mt-0.5 line-clamp-3 select-text whitespace-pre-wrap text-[0.8125rem] leading-relaxed text-fg-muted [overflow-wrap:anywhere]">
              {notice.content}
            </p>
          )}

          {(notice.canReply || actions.length > 0) && (
            <div className="mt-3 flex flex-wrap gap-1.5">
              {notice.canReply && (
                <Button
                  size="sm"
                  variant={replying ? "primary" : "secondary"}
                  icon={<Reply size={12} />}
                  onClick={() => setReplying((v) => !v)}
                  aria-expanded={replying}
                >
                  Reply
                </Button>
              )}
              {actions.map((action) => (
                <AsyncButton key={action.id} size="sm" variant="secondary" onClick={() => actOnNotice(notice.key, action.id)}>
                  {action.name}
                </AsyncButton>
              ))}
            </div>
          )}

          <AnimatePresence initial={false}>
            {replying && <ReplyComposer noticeKey={notice.key} to={notice.title || app} onDone={() => setReplying(false)} />}
          </AnimatePresence>
        </div>
      </article>
    </motion.li>
  );
});
