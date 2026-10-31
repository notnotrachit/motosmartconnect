import { useEffect, useState } from "react";
import {
  Bell, Bluetooth, Check, ChevronRight, Link2, MessageCircle, Monitor,
  Play, Power, QrCode, Settings, Smartphone, Wifi, ArrowUpRight,
} from "lucide-react";
import { getCurrentWindow, Window } from "@tauri-apps/api/window";
import { emit } from "@tauri-apps/api/event";
import { applySnapshot } from "@/engine/sync";
import { api, errorText, events } from "@/lib/tauri";
import { useConnectedPhone, useSnapshot } from "@/store/engine";
import type { Snapshot } from "@/lib/types";
import { useApplyPrefs } from "@/hooks/useApplyPrefs";
import { cn } from "@/lib/cn";
import { AppIcon } from "@/components/ui/AppIcon";
import { iconFor, useIcons, useNotifications } from "@/store/engine";

const quick = "group flex min-w-0 flex-1 flex-col items-center gap-2 text-[11px] font-medium text-fg-muted transition hover:text-fg disabled:cursor-default disabled:opacity-40";

export function TrayPanel() {
  useApplyPrefs();
  const snapshot = useSnapshot();
  const phone = useConnectedPhone();
  const notices = useNotifications();
  const icons = useIcons();
  const [busy, setBusy] = useState("");
  const [error, setError] = useState("");
  const paired = (snapshot?.phones.length ?? 0) > 0;
  const active = Boolean(phone);

  useEffect(() => {
    let cancelled = false;
    let unlisten: (() => void) | undefined;
    let poll: number | undefined;
    const refresh = () => { void api.snapshot().then(applySnapshot).catch(() => {}); };
    const onFocus = () => {
      refresh();
      if (poll === undefined) poll = window.setInterval(refresh, 1000);
    };
    const onBlur = () => {
      if (poll !== undefined) window.clearInterval(poll);
      poll = undefined;
    };
    refresh();
    void events.onSnapshot(applySnapshot).then((stop) => {
      if (cancelled) stop();
      else unlisten = stop;
    });
    window.addEventListener("focus", onFocus);
    window.addEventListener("blur", onBlur);
    return () => {
      cancelled = true;
      onBlur();
      window.removeEventListener("focus", onFocus);
      window.removeEventListener("blur", onBlur);
      unlisten?.();
    };
  }, []);

  const action = async (id: string, task: () => Promise<unknown>) => {
    setBusy(id);
    setError("");
    try {
      const result = await task();
      if (result && typeof result === "object" && "phones" in result) applySnapshot(result as Snapshot);
    } catch (reason) {
      setError(errorText(reason));
    } finally {
      setBusy("");
    }
  };

  const openMain = async (destination?: string) => {
    const main = await Window.getByLabel("main");
    await main?.show();
    await main?.unminimize();
    if (destination) await emit("tray:navigate", destination);
    await getCurrentWindow().hide();
    await main?.setFocus();
  };

  return (
    <div className="h-full p-2.5 text-fg">
      <div className="flex h-full flex-col overflow-hidden rounded-[24px] border border-white/10 bg-[linear-gradient(145deg,#263449_0%,#20252d_56%,#292c31_100%)] p-3.5 shadow-pop">
        <header className="flex items-center justify-between px-1 pb-3 text-[13px] font-semibold">
          <span>Smart Connect</span>
          <span className="flex items-center gap-1.5 text-xs font-medium text-fg-muted">
            <i className={cn("size-2 rounded-full", active ? "bg-success" : "bg-warning")} />
            {active ? "Connected" : paired ? "Paired" : "Not paired"}
          </span>
        </header>

        <section className="rounded-[20px] border border-white/10 bg-white/[0.055] p-4">
          <div className="flex items-center gap-4">
            <Smartphone size={48} strokeWidth={1.7} className="shrink-0 text-white/45" />
            <div className="min-w-0 flex-1">
              <div className="truncate text-[17px] font-semibold text-white">
                {phone?.name || snapshot?.phones[0]?.name || "No phone connected"}
              </div>
              <div className="mt-2 flex flex-wrap gap-1.5">
                <span className="inline-flex items-center gap-1.5 rounded-full bg-white/[0.08] px-2.5 py-1 text-[11px] font-medium text-white/65">
                  <Wifi size={12} /> {active ? "Connected" : "Offline"}
                </span>
                {snapshot?.messages.unread ? (
                  <span className="inline-flex items-center gap-1.5 rounded-full bg-white/[0.08] px-2.5 py-1 text-[11px] font-medium text-white/65">
                    <MessageCircle size={12} /> {snapshot.messages.unread} unread
                  </span>
                ) : null}
              </div>
            </div>
          </div>

          <div className="mt-4 flex justify-between gap-1 px-1">
            <button className={quick} onClick={() => void (paired ? action("connect", api.reconnect).then(() => openMain()) : action("pair", api.beginPairing))}>
              <span className="grid size-[46px] place-items-center rounded-full border border-white/10 bg-white/[0.07] text-white transition group-hover:bg-white/[0.13]"><Link2 size={21} /></span>
              {paired ? "Connect" : "Pair"}
            </button>
            <button className={quick} disabled={!active || Boolean(busy)} onClick={() => void openMain("mirror")}>
              <span className="grid size-[46px] place-items-center rounded-full border border-white/10 bg-white/[0.07] text-white transition group-hover:bg-white/[0.13]"><Play size={19} fill="currentColor" /></span>
              Mirror
            </button>
            <button className={quick} disabled={!active || Boolean(busy)} onClick={() => void action("desktop", async () => { await api.openStreamWindow("desktop", "Phone desktop"); return api.startDesktop(); })}>
              <span className="grid size-[46px] place-items-center rounded-full border border-white/10 bg-white/[0.07] text-white transition group-hover:bg-white/[0.13]"><Monitor size={21} /></span>
              Desktop
            </button>
            <button className={quick} onClick={() => void openMain("messages")}>
              <span className="grid size-[46px] place-items-center rounded-full border border-white/10 bg-white/[0.07] text-white transition group-hover:bg-white/[0.13]"><MessageCircle size={21} /></span>
              SMS
            </button>
            <button className={quick} onClick={() => void openMain("notifications")}>
              <span className="grid size-[46px] place-items-center rounded-full border border-white/10 bg-white/[0.07] text-white transition group-hover:bg-white/[0.13]"><Bell size={21} /></span>
              Alerts
            </button>
          </div>
        </section>

        <div className="mt-2.5 flex min-h-9 items-center gap-2 rounded-xl border border-white/[0.07] bg-black/10 px-3 text-[11px] text-white/60">
          {active ? <Check size={14} className="text-success" /> : <Bluetooth size={14} className="text-warning" />}
          <span className="truncate">{error || (busy ? "Starting…" : active ? "Your phone is ready to use." : paired ? "Looking for your phone. It can stay in the background if Bluetooth is on." : "Pair a phone to get started.")}</span>
        </div>

        <section className="mt-2.5 min-h-0 flex-1 overflow-hidden" aria-label="Recent notifications">
          <div className="mb-1 flex items-center justify-between px-1">
            <h2 className="text-[11px] font-semibold text-white/65">Recent notifications</h2>
            <button onClick={() => void openMain("notifications")} className="flex items-center gap-1 text-[10px] font-medium text-white/45 hover:text-white/80">
              See all <ArrowUpRight size={11} />
            </button>
          </div>
          {notices.length ? (
            <ul className="flex flex-col">
              {notices.slice(0, 3).map((notice) => (
                <li key={notice.key}>
                  <button
                    onClick={() => void action("notification", () => api.notificationOpen(notice.key))}
                    className="flex w-full items-center gap-2.5 rounded-lg px-1.5 py-1 text-left transition hover:bg-white/[0.07]"
                    aria-label={`Open notification from ${notice.app || "Phone"}: ${notice.title || notice.content}`}
                  >
                    <AppIcon src={notice.icon || iconFor(notice.pkg, snapshot, icons)} name={notice.app || "Phone"} size={25} />
                    <span className="min-w-0 flex-1">
                      <span className="block truncate text-[10px] font-medium leading-tight text-white/65">{notice.app || "Phone"}</span>
                      <span className="block truncate text-[11px] font-semibold leading-tight text-white/90">{notice.title || notice.content || "Notification"}</span>
                      {notice.title && notice.content && <span className="block truncate text-[10px] leading-tight text-white/55">{notice.content}</span>}
                    </span>
                  </button>
                </li>
              ))}
            </ul>
          ) : (
            <p className="px-1.5 py-1 text-[10px] text-white/40">New notifications from your phone will appear here.</p>
          )}
        </section>

        <nav className="mt-auto flex shrink-0 flex-col gap-0.5 pt-2">
          <MenuRow icon={<Monitor size={15} />} label="Open Smart Connect" onClick={() => void openMain()} />
          <MenuRow icon={<QrCode size={15} />} label="Pair new phone…" onClick={() => void action("pair", api.beginPairing).then(() => openMain("pair"))} />
          <MenuRow icon={<Settings size={15} />} label="Settings…" onClick={() => void openMain("settings")} />
          <MenuRow icon={<Power size={15} />} label="Quit Smart Connect" onClick={() => void action("quit", () => api.quit())} />
        </nav>
      </div>
    </div>
  );
}

function MenuRow({ icon, label, onClick }: { icon: React.ReactNode; label: string; onClick: () => void }) {
  return (
    <button onClick={onClick} className="flex h-8 items-center gap-3 rounded-lg px-2.5 text-left text-[12px] font-medium text-white/85 transition hover:bg-white/10">
      <span className="text-white/70">{icon}</span>{label}<ChevronRight size={13} className="ml-auto text-white/25" />
    </button>
  );
}
