import { useEffect, useState, type FormEvent } from "react";
import { Button } from "@/components/ui/Button";
import { StatusDot } from "@/components/ui/Feedback";
import { Input } from "@/components/ui/Input";
import { Switch } from "@/components/ui/Switch";
import { setDeviceName } from "@/engine/actions";
import { useEngine } from "@/store/engine";
import { useUi } from "@/store/ui";
import { CopyButton } from "@/components/ui/CopyButton";
import { SettingGroup, SettingRow } from "./SettingRow";

export function GeneralSection() {
  const snapshot = useEngine((s) => s.snapshot);
  const prefs = useUi((s) => s.prefs);
  const setPref = useUi((s) => s.setPref);
  const saved = snapshot?.deviceName ?? "";
  const [name, setName] = useState(saved);
  const [saving, setSaving] = useState(false);

  // Follow the engine's value until the user starts editing.
  useEffect(() => setName(saved), [saved]);

  const dirty = name.trim() !== saved && name.trim().length > 0;
  const save = async (event: FormEvent) => {
    event.preventDefault();
    if (!dirty) return;
    setSaving(true);
    await setDeviceName(name);
    setSaving(false);
  };

  const address = snapshot?.lanIp ? `${snapshot.lanIp}:${snapshot.port}` : "—";
  const otherIps = (snapshot?.ips ?? []).filter((ip) => ip !== snapshot?.lanIp);

  return (
    <>
      <SettingGroup title="This Mac">
        <form onSubmit={save} className="px-4 py-3">
          <label htmlFor="device-name" className="block text-[0.8125rem] font-medium text-fg">
            Name on the phone
          </label>
          <p className="mt-0.5 text-xs text-fg-muted">Smart Connect shows this name for the computer.</p>
          <div className="mt-2.5 flex gap-2">
            <Input
              id="device-name"
              data-autofocus
              value={name}
              onChange={(event) => setName(event.target.value)}
              maxLength={40}
              autoComplete="off"
              wrapperClassName="flex-1"
              trailing={<span className="pr-1 font-mono text-[10.5px] tabular-nums text-fg-subtle">{name.length}/40</span>}
            />
            <Button type="submit" variant="primary" disabled={!dirty} loading={saving}>
              Save
            </Button>
          </div>
        </form>
      </SettingGroup>

      <SettingGroup title="Network">
        <SettingRow label="Listener" description="The phone connects to this Mac on the local network.">
          <span className="flex items-center gap-2 text-xs text-fg-muted">
            <StatusDot tone={snapshot?.listening ? "live" : "warn"} pulse />
            {snapshot?.listening ? "Listening" : "Starting"}
          </span>
        </SettingRow>
        <SettingRow label="Address" description={otherIps.length ? `Also on ${otherIps.join(", ")}` : undefined}>
          <span className="select-text font-mono text-xs text-fg-muted">{address}</span>
          {snapshot?.lanIp && <CopyButton value={address} label="Copy address" />}
        </SettingRow>
        <SettingRow label="Device ID" description="Identifies this Mac to paired phones.">
          <span className="max-w-[180px] select-text truncate font-mono text-xs text-fg-muted">{snapshot?.realDeviceId || "—"}</span>
          {snapshot?.realDeviceId && <CopyButton value={snapshot.realDeviceId} label="Copy device ID" />}
        </SettingRow>
      </SettingGroup>

      <SettingGroup title="Alerts">
        <SettingRow label="Connection changes" description="Show a toast when the phone connects or drops.">
          <Switch label="Connection changes" checked={prefs.connectionToasts} onChange={(v) => setPref("connectionToasts", v)} />
        </SettingRow>
        <SettingRow label="Received files" description="Show a toast when the phone sends a file.">
          <Switch label="Received files" checked={prefs.fileToasts} onChange={(v) => setPref("fileToasts", v)} />
        </SettingRow>
        <SettingRow label="New texts" description="Show a toast when a text arrives in a conversation you aren't viewing.">
          <Switch label="New texts" checked={prefs.messageToasts} onChange={(v) => setPref("messageToasts", v)} />
        </SettingRow>
        <SettingRow label="Open app when a banner is clicked" description="Bring up the phone app stream and activate that notification on the phone.">
          <Switch label="Open phone app from banner" checked={prefs.openNotificationOnClick} onChange={(v) => setPref("openNotificationOnClick", v)} />
        </SettingRow>
        <SettingRow label="Confirm before forgetting" description="Ask before removing a paired phone.">
          <Switch label="Confirm before forgetting" checked={prefs.confirmForget} onChange={(v) => setPref("confirmForget", v)} />
        </SettingRow>
      </SettingGroup>
    </>
  );
}
