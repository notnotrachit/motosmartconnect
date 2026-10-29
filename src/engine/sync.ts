import { api, errorText, events } from "@/lib/tauri";
import type { Snapshot } from "@/lib/types";
import { useEngine } from "@/store/engine";
import { toast } from "@/store/toast";
import { useUi } from "@/store/ui";
import { useMessageUi } from "@/store/messages";
import { mirror } from "./mirror";
import { cameraWindowClosed, openThread, reveal, showStream, streamIsOpen } from "./actions";

/**
 * Single entry point for engine data. Every snapshot, whether pushed by the
 * "snapshot" event or returned from a command, goes through applySnapshot.
 */

let started = false;
let iconsInFlight = false;

export function applySnapshot(snapshot: Snapshot) {
  const previous = useEngine.getState().snapshot;
  const { qrLifetime } = useEngine.getState();
  const newCode = !previous || previous.qrSvg !== snapshot.qrSvg;
  useEngine.setState({
    snapshot,
    qrDeadline: Date.now() + (snapshot.qrExpiresIn ?? 0) * 1000,
    qrLifetime: newCode ? Math.max(1, snapshot.qrExpiresIn) : Math.max(qrLifetime, snapshot.qrExpiresIn),
  });

  mirror().onSnapshot(snapshot);
  announce(previous, snapshot);
  void refreshIcons();
  followCamera(snapshot);
}

/** Toasts for things that happened between two snapshots. Silent on the first one. */
function announce(previous: Snapshot | null, next: Snapshot) {
  if (next.error && next.error !== previous?.error) toast.error(next.error);
  if (!previous) return;
  const prefs = useUi.getState().prefs;

  const before = previous.phones ?? [];
  const after = next.phones ?? [];
  if (!before.length && after.length) {
    toast.success(`${after[0].name || "Phone"} is paired`, { body: "Notifications, apps and the screen are ready." });
  } else if (prefs.connectionToasts) {
    for (const phone of after) {
      const was = before.find((p) => p.realId === phone.realId);
      if (!was) continue;
      if (!was.connected && phone.connected) toast.success(`${phone.name || "Phone"} connected`);
      if (was.connected && !phone.connected) {
        toast.warning(`${phone.name || "Phone"} disconnected`, {
          body: phone.userDisconnected ? "It was disconnected on the phone." : "Open Smart Connect on the phone to reconnect.",
        });
      }
    }
  }

  if (prefs.fileToasts) {
    const known = new Set((previous.files ?? []).map((f) => f.path));
    for (const file of next.files ?? []) {
      if (known.has(file.path)) continue;
      toast.success(`Received ${file.name}`, {
        body: "Saved to Downloads › Smart Connect",
        action: { label: "Show in Finder", run: () => void reveal(file.path) },
      });
    }
  }

  if (prefs.messageToasts) announceTexts(previous, next);
}

/**
 * A toast for each conversation whose unread count went up, unless that
 * conversation is already on screen. Silent while the history is still loading.
 */
function announceTexts(previous: Snapshot, next: Snapshot) {
  const before = previous.messages;
  const after = next.messages;
  if (!before || !after || before.phase !== "ready" || after.phase !== "ready") return;
  const viewing = useUi.getState().tab === "messages" && document.hasFocus() ? after.openKey : "";
  const was = new Map(before.threads.map((t) => [t.key, t.unread]));
  for (const thread of after.threads) {
    if (thread.unread <= (was.get(thread.key) ?? 0) || thread.key === viewing) continue;
    toast.info(thread.title, {
      body: thread.preview,
      action: {
        label: "Reply",
        run: () => {
          useUi.getState().setTab("messages");
          useMessageUi.getState().stopCompose();
          void openThread(thread.key);
        },
      },
    });
  }
}

/** Fetch app icons when the engine reports a new icon revision. */
export async function refreshIcons() {
  const { snapshot, icons } = useEngine.getState();
  if (!snapshot || iconsInFlight || snapshot.iconsRevision === icons.revision) return;
  if (!snapshot.apps?.length && !snapshot.notifications?.length) return;
  iconsInFlight = true;
  try {
    useEngine.setState({ icons: await api.appIcons() });
  } catch {
    /* Letters stand in for icons. */
  } finally {
    iconsInFlight = false;
  }
}

/** The phone can start the camera on its own; open its window once when it does. */
function followCamera(snapshot: Snapshot) {
  const camera = (snapshot.sessions ?? []).find((session) => session.pkg === "webcam");
  if (!camera) {
    cameraWindowClosed();
    return;
  }
  if (!streamIsOpen("webcam")) void showStream("webcam", "Phone camera");
}

export function startEngineSync() {
  if (started) return;
  started = true;
  const screen = mirror();
  screen.start();
  void events.onSnapshot(applySnapshot);
  void events.onWallpaper((url) => {
    useEngine.setState({ wallpaper: url.startsWith("data:image/") ? url : "" });
  });
  api.wallpaper().then((url) => {
    if (url.startsWith("data:image/")) useEngine.setState({ wallpaper: url });
  }).catch(() => {});
  void events.onRtc((signal) => void screen.onRtc(signal));
  void events.onFrame((frame) => screen.onFrame(frame));
  api
    .pageOffers()
    .then((offers) => {
      for (const offer of offers || []) void screen.onRtc(offer);
    })
    .catch(() => {});
  api
    .snapshot()
    .then(applySnapshot)
    .catch((error) => toast.error("Smart Connect could not start", { body: errorText(error) }));
}
