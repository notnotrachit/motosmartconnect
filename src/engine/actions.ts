import { api, errorText } from "@/lib/tauri";
import type { AppView, NoticeView, PhoneEntry, Snapshot } from "@/lib/types";
import { useEngine } from "@/store/engine";
import { toast } from "@/store/toast";
import { mirror } from "./mirror";
import { applySnapshot } from "./sync";

/**
 * Every user-triggered engine command. Components call these and never talk to
 * Tauri directly, so error handling and snapshot application stay uniform.
 * Functions resolve to true on success and false after showing an error.
 */

const streamOpened = new Set<string>();

export const streamIsOpen = (pkg: string) => streamOpened.has(pkg);
export const cameraWindowClosed = () => streamOpened.delete("webcam");

async function run(command: () => Promise<Snapshot>, ignore?: (message: string) => boolean): Promise<boolean> {
  try {
    applySnapshot(await command());
    return true;
  } catch (error) {
    const message = errorText(error);
    if (!ignore?.(message)) toast.error(message);
    return false;
  }
}

const stillWaiting = (message: string) => message.includes("Still waiting");

// ------------------------------------------------------------------ pairing

export function refreshCode() {
  const pairing = useEngine.getState().snapshot?.pairing;
  return run(pairing ? api.refreshQr : api.beginPairing);
}

export const beginPairing = () => run(api.beginPairing);
export const endPairing = () => run(api.endPairing);
export const forgetPhone = (realId: string) => run(() => api.unpair(realId));

export async function setDeviceName(name: string) {
  const ok = await run(() => api.setName(name.trim()));
  if (ok) toast.success("Name saved", { body: `The phone now sees this Mac as “${name.trim()}”.` });
  return ok;
}

// --------------------------------------------------------------- messages

export const openThread = (key: string) => run(() => api.messagesOpen(key));
export const closeThread = () => run(() => api.messagesClose());
export const sendThread = (text: string) => run(() => api.messagesSend(text));
export const sendNewThread = (address: string, text: string) => run(() => api.messagesSendNew(address, text));

// ------------------------------------------------------------ notifications

export async function replyToNotice(key: string, text: string) {
  if (!text.trim()) return false;
  const ok = await run(() => api.notificationReply(key, text));
  if (ok) toast.success("Reply sent");
  return ok;
}

export const actOnNotice = (key: string, actionId: number) => run(() => api.notificationAct(key, actionId));

export async function openNotice(notice: NoticeView) {
  if (notice.pkg) await showStream(notice.pkg, notice.app || notice.pkg);
  return run(() => api.notificationOpen(notice.key));
}

/** Open a macOS-banner target in its stream window, then activate it on the phone. */
export async function openBannerNotice(key: string, pkgFromBanner: string) {
  const snapshot = useEngine.getState().snapshot;
  const notice = snapshot?.notifications.find((item) => item.key === key);
  const app = snapshot?.apps.find((item) => item.pkg === (notice?.pkg || pkgFromBanner))
    ?? snapshot?.apps.find((item) => item.name === notice?.app);
  const pkg = notice?.pkg || pkgFromBanner || app?.pkg || "";
  const title = notice?.app || app?.name || pkg || "Phone app";

  if (pkg) await showStream(pkg, title);

  if (notice) {
    const opened = await run(() => api.notificationOpen(key));
    // A notification can omit its package even when the installed app list has it.
    if (opened && !notice.pkg && app) return run(() => api.startApp(app.pkg, app.cmp, app.userId));
    return opened;
  }

  // The phone may have removed the notification before its macOS banner was clicked.
  if (app) return run(() => api.startApp(app.pkg, app.cmp, app.userId));
  if (pkg) return run(() => api.startApp(pkg, "", 0));
  return false;
}

// ------------------------------------------------------------------ streams

export async function showStream(pkg: string, title: string) {
  if (!pkg) return false;
  try {
    await api.openStreamWindow(pkg, title || "Phone app");
    streamOpened.add(pkg);
    return true;
  } catch (error) {
    streamOpened.delete(pkg);
    toast.error(errorText(error));
    return false;
  }
}

export async function openApp(app: AppView) {
  const name = app.name || app.pkg;
  await showStream(app.pkg, name);
  return run(() => api.startApp(app.pkg, app.cmp ?? "", app.userId ?? 0));
}

export function startMirror() {
  mirror().beginMirror();
  return run(api.startMirror, (message) => {
    if (!message.includes("already opening") && !stillWaiting(message)) mirror().resetIdle();
    return stillWaiting(message);
  });
}

export async function startDesktop() {
  await showStream("desktop", "Phone desktop");
  return run(api.startDesktop, stillWaiting);
}

export async function startWebcam() {
  streamOpened.delete("webcam");
  await showStream("webcam", "Phone camera");
  return run(api.startWebcam);
}

export async function stopStream() {
  try {
    await mirror().releaseHeld("screen");
  } catch {
    /* Releasing is best effort. */
  }
  return run(() => api.stopStream(mirror().currentSession));
}

export function showSession(sessionId: string, pkg: string, title: string) {
  if (pkg && pkg !== "mirror") void showStream(pkg, title);
  else mirror().showSession(sessionId);
}

export async function pressAndroidKey(code: number) {
  await mirror().pressAndroidKey(code).catch(() => {});
}

export async function typeOnPhone(text: string) {
  await mirror().typeText(text).catch((error) => toast.error(errorText(error)));
}

// ------------------------------------------------------------ cross control

export const crossStart = () => run(api.crossStart);

export async function crossStop() {
  try {
    await mirror().releaseHeld("cross");
    await api.crossShow(false);
  } catch {
    /* Stopping still goes ahead. */
  }
  return run(api.crossStop);
}

// --------------------------------------------------------------------- apps

export const refreshApps = () => run(api.refreshApps);

// -------------------------------------------------------------------- files

export function listPhoneFolder(path: string): Promise<PhoneEntry[]> {
  return api.listPhone(path);
}

export async function savePhoneFile(path: string) {
  try {
    const saved = await api.savePhone(path);
    const name = path.split("/").pop() || "File";
    toast.success(`Saved ${name}`, {
      body: "Downloads › Smart Connect",
      action: { label: "Show in Finder", run: () => void reveal(saved) },
    });
    return true;
  } catch (error) {
    toast.error(errorText(error));
    return false;
  }
}

export async function reveal(path: string) {
  try {
    await api.revealFile(path);
  } catch (error) {
    toast.error(errorText(error));
  }
}
