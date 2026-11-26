import { useEffect, useState, type FormEvent } from "react";
import { Button } from "@/components/ui/Button";
import { StatusDot } from "@/components/ui/Feedback";
import { Input } from "@/components/ui/Input";
import { api, errorText, events, type AccountView } from "@/lib/tauri";
import { SettingGroup, SettingRow } from "./SettingRow";

const EMPTY: AccountView = {
  signedIn: false,
  name: "",
  detail: "",
  devices: [],
};

export function AccountSection() {
  const [account, setAccount] = useState<AccountView>(EMPTY);
  const [callback, setCallback] = useState("");
  const [busy, setBusy] = useState<"in" | "out" | "refresh" | "paste" | "connect" | null>(null);
  const [notice, setNotice] = useState("");

  useEffect(() => {
    let stop = false;
    void api.accountStatus().then((view) => {
      if (!stop) setAccount(view);
    }).catch((error) => {
      if (!stop) setNotice(errorText(error));
    });
    let unlisten: (() => void) | undefined;
    void events.onAccount((view) => {
      setAccount(view);
      setNotice("");
      setBusy(null);
    }).then((fn) => {
      if (stop) fn();
      else unlisten = fn;
    });
    return () => {
      stop = true;
      unlisten?.();
    };
  }, []);

  const run = async (kind: NonNullable<typeof busy>, action: () => Promise<AccountView>) => {
    setBusy(kind);
    setNotice("");
    try {
      setAccount(await action());
    } catch (error) {
      setNotice(errorText(error));
      try {
        setAccount(await api.accountStatus());
      } catch {
        // The thrown message is enough when status itself fails.
      }
    } finally {
      setBusy(null);
    }
  };

  const paste = (event: FormEvent) => {
    event.preventDefault();
    const url = callback.trim();
    if (!url) return;
    setCallback("");
    void run("paste", () => api.accountFinish(url));
  };

  const label = account.name || (account.signedIn ? "Signed in" : "Not signed in");

  return (
    <>
      <SettingGroup
        title="Lenovo or Motorola account"
        description="Same account as the phone, with cloud connectivity on. Pair on Wi-Fi once before screen and messages can use this path. A connection off this network still has to be checked with the phone away from this Wi-Fi."
      >
        <SettingRow label={label} description={account.detail || "Loading account status."}>
          <span className="flex items-center gap-2 text-xs text-fg-muted">
            <StatusDot tone={account.signedIn ? "live" : "idle"} />
            {account.signedIn ? "Signed in" : "Signed out"}
          </span>
        </SettingRow>
        <SettingRow label="Sign-in" description="Opens Lenovo in the browser. A development build often cannot catch smartconnect:authcode, so paste the address instead.">
          {account.signedIn ? (
            <Button variant="danger" loading={busy === "out"} disabled={busy !== null} onClick={() => void run("out", () => api.accountSignOut())}>
              Sign out
            </Button>
          ) : (
            <Button variant="primary" loading={busy === "in"} disabled={busy !== null} onClick={() => void run("in", () => api.accountSignIn())}>
              Sign in
            </Button>
          )}
        </SettingRow>
        {!account.signedIn && (
          <form onSubmit={paste} className="px-4 py-3">
            <label htmlFor="account-callback" className="block text-[0.8125rem] font-medium text-fg">
              Browser did not return
            </label>
            <p className="mt-0.5 text-xs text-fg-muted">
              Paste the full passport.lenovo.com address from the address bar, or a smartconnect:authcode address.
            </p>
            <div className="mt-2.5 flex gap-2">
              <Input
                id="account-callback"
                value={callback}
                onChange={(event) => setCallback(event.target.value)}
                autoComplete="off"
                spellCheck={false}
                placeholder="https://passport.lenovo.com/… or smartconnect:authcode?code=…"
                wrapperClassName="min-w-0 flex-1"
              />
              <Button type="submit" variant="secondary" loading={busy === "paste"} disabled={busy !== null || callback.trim().length === 0}>
                Continue
              </Button>
            </div>
          </form>
        )}
      </SettingGroup>

      {account.signedIn && (
        <SettingGroup title="Devices on this account" description="Connect asks the cloud broker for a tunnel into this Mac. Wi-Fi and Bluetooth wake stay available.">
          <SettingRow label="Device list" description={account.devices.length === 0 ? "No devices yet." : `${account.devices.length} listed`}>
            <Button
              variant="secondary"
              size="sm"
              loading={busy === "refresh"}
              disabled={busy !== null}
              onClick={() => void run("refresh", () => api.accountRefreshDevices())}
            >
              Refresh
            </Button>
          </SettingRow>
          {account.devices.map((device) => (
            <SettingRow
              key={device.id}
              label={device.name || "Unnamed device"}
              description={device.online ? "The account list marks this device online." : "Listed on this account."}
            >
              <Button
                variant="primary"
                size="sm"
                loading={busy === "connect"}
                disabled={busy !== null}
                onClick={() => void run("connect", () => api.accountConnect(device.id))}
              >
                Connect
              </Button>
            </SettingRow>
          ))}
        </SettingGroup>
      )}

      {notice && <p className="text-xs leading-relaxed text-danger">{notice}</p>}
    </>
  );
}
