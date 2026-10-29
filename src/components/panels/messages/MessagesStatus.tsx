import { MessageSquareOff, ShieldAlert, Smartphone, WifiOff } from "lucide-react";
import { Skeleton } from "@/components/ui/Feedback";
import { cn } from "@/lib/cn";

export type MessagesPhase = "waiting" | "denied" | "off" | "loading" | "ready";

const COPY: Record<Exclude<MessagesPhase, "ready" | "loading">, { icon: typeof WifiOff; title: string; body: string; tone: "warn" | "muted" }> = {
  waiting: { icon: Smartphone, title: "Waiting for the phone", body: "Texts show up here once the phone connects.", tone: "muted" },
  denied: {
    icon: ShieldAlert,
    title: "Messages are blocked",
    body: "Allow SMS access for Smart Connect in the phone's app settings, then reconnect.",
    tone: "warn",
  },
  off: {
    icon: MessageSquareOff,
    title: "Turn on Messages on the phone",
    body: "Open Smart Connect on the phone and allow Messages for this Mac.",
    tone: "warn",
  },
};

/** Explains why there are no conversations yet, in place of the list. */
export function MessagesStatus({ phase, connected }: { phase: MessagesPhase; connected: boolean }) {
  if (phase === "loading") return <ThreadSkeletons />;
  if (phase === "ready") return null;
  const copy = !connected ? { ...COPY.waiting, icon: WifiOff, title: "Phone is offline", body: "Connect the phone to read and send texts." } : COPY[phase];
  const Icon = copy.icon;
  return (
    <div className="flex flex-col items-center px-6 py-12 text-center">
      <span
        className={cn(
          "mb-3 grid size-11 place-items-center rounded-2xl ring-1",
          copy.tone === "warn" ? "bg-warning/10 text-warning ring-warning/25" : "bg-surface-2 text-fg-muted ring-line",
        )}
      >
        <Icon size={19} />
      </span>
      <p className="text-[0.8125rem] font-semibold text-fg">{copy.title}</p>
      <p className="mt-1 max-w-[30ch] text-xs leading-relaxed text-fg-muted">{copy.body}</p>
    </div>
  );
}

export function ThreadSkeletons() {
  return (
    <ul aria-label="Loading conversations" className="flex flex-col gap-1 px-2.5 py-1">
      {Array.from({ length: 7 }, (_, i) => (
        <li key={i} className="flex items-center gap-3 py-2.5" style={{ opacity: 1 - i * 0.11 }}>
          <Skeleton className="size-[38px] rounded-full" />
          <div className="flex-1 space-y-2">
            <Skeleton className="h-3 w-2/5" />
            <Skeleton className="h-2.5 w-4/5" />
          </div>
        </li>
      ))}
    </ul>
  );
}
