import { useEffect, useState } from "react";
import { AlertTriangle } from "lucide-react";
import { Button } from "@/components/ui/Button";
import { Dialog } from "@/components/ui/Dialog";
import { settleConfirm, useConfirm, type ConfirmOptions } from "@/store/confirm";
import { cn } from "@/lib/cn";

export function ConfirmDialog() {
  const request = useConfirm((s) => s.request);
  // Keep the last content on screen while the exit animation plays.
  const [shown, setShown] = useState<ConfirmOptions | null>(request);
  useEffect(() => {
    if (request) setShown(request);
  }, [request]);

  const danger = shown?.tone === "danger";
  return (
    <Dialog open={Boolean(request)} onClose={() => settleConfirm(false)} size="sm" hideHeader>
      <div className="flex gap-4 p-5">
        <div
          className={cn(
            "grid size-10 shrink-0 place-items-center rounded-xl",
            danger ? "bg-danger/10 text-danger" : "bg-accent/15 text-accent",
          )}
        >
          <AlertTriangle size={18} />
        </div>
        <div className="min-w-0">
          <h2 className="text-[0.95rem] font-semibold text-fg">{shown?.title}</h2>
          {shown?.body && <p className="mt-1.5 text-[0.8125rem] leading-relaxed text-fg-muted">{shown.body}</p>}
        </div>
      </div>
      <footer className="flex justify-end gap-2 border-t border-line bg-surface-2/50 px-5 py-3">
        <Button variant="ghost" onClick={() => settleConfirm(false)}>
          Cancel
        </Button>
        <Button
          data-autofocus
          variant={danger ? "danger" : "primary"}
          className={danger ? "bg-danger text-white hover:bg-danger/90 border-transparent" : undefined}
          onClick={() => settleConfirm(true)}
        >
          {shown?.confirmLabel ?? "Confirm"}
        </Button>
      </footer>
    </Dialog>
  );
}
