import { useDeferredValue, useMemo, useState } from "react";
import { AnimatePresence } from "motion/react";
import { BellRing, Search, SearchX, WifiOff } from "lucide-react";
import { Button } from "@/components/ui/Button";
import { EmptyState } from "@/components/ui/Feedback";
import { Input } from "@/components/ui/Input";
import { fuzzyScore } from "@/lib/format";
import { iconFor, useConnectedPhone, useIcons, useNotifications, useSnapshot } from "@/store/engine";
import { PanelHeader } from "../PanelHeader";
import { NotificationCard } from "./NotificationCard";

export function NotificationsPanel() {
  const notices = useNotifications();
  const snapshot = useSnapshot();
  const icons = useIcons();
  const connected = Boolean(useConnectedPhone());
  const [query, setQuery] = useState("");
  const deferred = useDeferredValue(query);

  const shown = useMemo(() => {
    if (!deferred.trim()) return notices;
    return notices.filter((n) => fuzzyScore(deferred, `${n.app} ${n.title} ${n.content}`) >= 0);
  }, [notices, deferred]);

  const subtitle = notices.length
    ? `${notices.length} from your phone${shown.length !== notices.length ? ` · ${shown.length} shown` : ""}`
    : "Live from your phone";

  return (
    <>
      <PanelHeader title="Notifications" subtitle={subtitle}>
        <Input
          value={query}
          onChange={(event) => setQuery(event.target.value)}
          onClear={() => setQuery("")}
          placeholder="Filter"
          aria-label="Filter notifications"
          leading={<Search size={13} />}
          wrapperClassName="w-52"
          disabled={!notices.length}
        />
      </PanelHeader>

      {!connected && notices.length > 0 && (
        <div className="mx-6 mb-3 flex items-center gap-2 rounded-xl border border-warning/25 bg-warning/10 px-3 py-2 text-xs text-warning">
          <WifiOff size={13} />
          The phone is offline. These are the last notifications it sent.
        </div>
      )}

      <div className="min-h-0 flex-1 overflow-y-auto px-6 pb-6">
        {!notices.length ? (
          <EmptyState
            icon={<BellRing size={20} />}
            title="You're all caught up"
            description={connected ? "New notifications from your phone appear here as they arrive." : "Notifications show up here once the phone connects."}
          />
        ) : !shown.length ? (
          <EmptyState icon={<SearchX size={20} />} title="No matches" description={`Nothing matches “${query}”.`}>
            <Button size="sm" onClick={() => setQuery("")}>
              Clear filter
            </Button>
          </EmptyState>
        ) : (
          <ul className="flex flex-col gap-2.5">
            <AnimatePresence initial={false} mode="popLayout">
              {shown.map((notice) => (
                <NotificationCard key={notice.key} notice={notice} icon={iconFor(notice.pkg, snapshot, icons)} />
              ))}
            </AnimatePresence>
          </ul>
        )}
      </div>
    </>
  );
}
