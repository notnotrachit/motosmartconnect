import { getCurrentWindow, LogicalSize } from "@tauri-apps/api/window";
import { ANDROID_FLAG, phoneButton } from "@/lib/keymap";
import { h264Profile, localCandidate, offerVideoMid, phoneCandidate, rewriteLocals, webkitH264, wireProfile } from "@/lib/sdp";
import { api, events } from "@/lib/tauri";
import type { InputKind, MirrorView, RtcSignal, SessionView, Snapshot } from "@/lib/types";
import { stripLogTime } from "@/lib/format";

/**
 * Drives one stream window (an app, the phone desktop, or the phone camera).
 * Port of the original stream.js: same signalling, same picture rules.
 *
 * WebKit blanks the picture if the <video> is moved or re-laid out after
 * playback starts, so the element is created here, mounted once, and never
 * touched by React afterwards.
 */

export type StreamKind = "app" | "desktop" | "webcam";

export interface StreamState {
  title: string;
  status: string;
  picture: boolean;
}

interface Peer {
  pc: RTCPeerConnection;
  sessionId: string;
  lanIp: string;
  offerMid: string;
  answered: boolean;
  draining: boolean;
  queue: { sessionId: string; candidate: string; label: number; mid: string }[];
}

interface Queued {
  kind: InputKind;
  x?: number;
  y?: number;
  key?: number;
  down?: boolean;
  flags?: number;
}

type Listener = (state: StreamState) => void;

const CAMERA_SINK = "http://127.0.0.1:59731/frame";

export class StreamController {
  readonly pkg: string;
  readonly kind: StreamKind;
  readonly video: HTMLVideoElement;
  readonly image: HTMLImageElement;
  private well: HTMLElement | null = null;

  private frameSeq = 0;
  private frameWidth = 0;
  private frameHeight = 0;
  private latest: Snapshot | null = null;
  private fitted = "";
  private revealed = false;
  private readonly peers = new Map<string, Peer>();
  private readonly earlyIce = new Map<string, RTCIceCandidateInit[]>();
  private holding: { finger: boolean; key?: number } | null = null;
  private readonly gesture: Queued[] = [];
  private sendingGesture = false;
  private started = false;

  private state: StreamState;
  private readonly listeners = new Set<Listener>();

  constructor(pkg: string) {
    this.pkg = pkg;
    this.kind = pkg === "webcam" ? "webcam" : pkg === "desktop" ? "desktop" : "app";
    const title = this.kind === "webcam" ? "Phone camera" : this.kind === "desktop" ? "Phone desktop" : "Phone app";
    this.state = {
      title,
      status: this.kind === "webcam" ? "Waiting for the phone camera." : "Waiting for the phone.",
      picture: false,
    };
    document.title = title;

    this.video = document.createElement("video");
    this.video.autoplay = true;
    this.video.playsInline = true;
    this.video.muted = true;
    this.video.className = "stream-video";
    this.video.addEventListener("resize", () => {
      if (this.video.videoWidth && this.video.videoHeight) this.showPicture(this.video.videoWidth, this.video.videoHeight);
    });

    this.image = document.createElement("img");
    this.image.alt = "";
    this.image.hidden = true;
    this.image.className = "stream-frame";
    this.image.addEventListener("load", () => {
      if (!this.image.getAttribute("src") || this.video.srcObject) return;
      this.showPicture(this.image.naturalWidth, this.image.naturalHeight);
    });
  }

  // ---------------------------------------------------------------- state

  getState = (): StreamState => this.state;

  subscribe = (fn: Listener): (() => void) => {
    this.listeners.add(fn);
    return () => this.listeners.delete(fn);
  };

  private emit(patch: Partial<StreamState>) {
    const next = { ...this.state, ...patch };
    if (next.title === this.state.title && next.status === this.state.status && next.picture === this.state.picture) return;
    this.state = next;
    for (const fn of this.listeners) fn(next);
  }

  private setStatus(text: string) {
    if (!text || this.state.picture) return;
    this.emit({ status: text });
  }

  // ------------------------------------------------------------- lifecycle

  /** Mount the picture into `host` once; input is bound to the surrounding `well`. */
  mount(well: HTMLElement, host: HTMLElement) {
    if (this.well === well) return;
    this.well = well;
    if (!this.video.isConnected) host.append(this.video, this.image);
    well.addEventListener("pointerdown", this.press);
    well.addEventListener("pointermove", this.drag);
    window.addEventListener("pointermove", this.drag);
    well.addEventListener("pointerup", this.release);
    window.addEventListener("pointerup", this.release);
    well.addEventListener("pointercancel", this.release);
    well.addEventListener("contextmenu", (event) => event.preventDefault());
  }

  start() {
    if (this.started) return;
    this.started = true;
    this.publishCamera();
    void this.boot();
    window.setInterval(async () => {
      let frame: MirrorView;
      try {
        frame = await api.mirrorFrame(this.frameSeq);
      } catch {
        return;
      }
      this.take(frame);
    }, 200);
  }

  private async boot() {
    await events.onSnapshot((snapshot) => {
      this.latest = snapshot;
      void this.applySession((snapshot.sessions ?? []).find((s) => this.mine(s)));
      this.showProgress(snapshot);
    });
    await events.onFrame((frame) => this.take(frame));
    await events.onRtc((signal) => this.onRtc(signal));
    await this.refresh();
    for (let attempt = 0; attempt < 12; attempt += 1) {
      try {
        const offers = await api.pageOffers();
        for (const offer of offers || []) this.onRtc(offer);
      } catch {
        /* The offer arrives on the rtc event when the phone sends it. */
      }
      if (this.peers.size > 0) break;
      await new Promise((resolve) => setTimeout(resolve, 250));
    }
  }

  private async refresh() {
    this.latest = await api.snapshot();
    await this.applySession((this.latest.sessions ?? []).find((s) => this.mine(s)));
    this.showProgress(this.latest);
  }

  // -------------------------------------------------------------- sessions

  private mine(session: SessionView | undefined): boolean {
    return Boolean(session && !session.quiet && session.pkg === this.pkg);
  }

  private async applySession(session: SessionView | undefined) {
    if (!session) return;
    const name = session.title || (this.kind === "webcam" ? "Phone camera" : "Phone app");
    document.title = name;
    this.emit({ title: name });
    try {
      await getCurrentWindow().setTitle(name);
    } catch {
      /* The document title is enough. */
    }
  }

  private showProgress(snapshot: Snapshot) {
    if (this.state.picture) return;
    const last = snapshot?.log?.at(-1);
    if (!last) return;
    const text = stripLogTime(last);
    if (this.kind === "webcam") {
      if (/phone camera/i.test(text)) this.setStatus(text);
      return;
    }
    if (/screen|picture|app|Opening|prepar|offer|decod|address|format|H264|video|Answered|blank|rejected/i.test(text)) {
      this.setStatus(text);
    }
  }

  // --------------------------------------------------------------- picture

  // The page must not resize the window after video starts; this runs for JPEG frames only.
  private async fitWindow(width: number, height: number) {
    if (!width || !height) return;
    const key = `${width}x${height}`;
    if (this.fitted === key) return;
    this.fitted = key;
    const nav = this.kind === "app" ? 52 : this.kind === "webcam" ? 44 : 0;
    const videoH = Math.min(820, Math.max(520, Math.round(height > width ? 780 : 480)));
    let w = Math.round(videoH * (width / height));
    w = Math.max(280, Math.min(w, 1100));
    const h = Math.round(w * (height / width)) + nav;
    try {
      await getCurrentWindow().setSize(new LogicalSize(w, h));
    } catch {
      /* The window can stay at its opening size. */
    }
  }

  private showPicture(width: number, height: number) {
    this.frameWidth = width || this.frameWidth;
    this.frameHeight = height || this.frameHeight;
    if (!this.revealed) {
      this.revealed = true;
      this.emit({ picture: true });
      const text = this.kind === "webcam" ? "The phone camera is showing." : "The app picture is showing.";
      api.screenNote(text).catch(() => {});
    }
  }

  private paint(jpeg: string, width: number, height: number) {
    if (!jpeg || this.video.srcObject) return;
    this.frameWidth = width || this.frameWidth;
    this.frameHeight = height || this.frameHeight;
    this.image.hidden = false;
    this.image.src = `data:image/jpeg;base64,${jpeg}`;
    void this.fitWindow(this.frameWidth, this.frameHeight);
  }

  private take(frame: MirrorView) {
    if (!frame?.jpeg || frame.seq === this.frameSeq) return;
    const session = (this.latest?.sessions ?? []).find((item) => item.sessionId === frame.sessionId);
    if (frame.sessionId && session && !this.mine(session)) return;
    if (frame.sessionId && !session && frame.sessionId !== "webcam" && this.pkg === "webcam") return;
    if (this.pkg === "webcam" && session && session.pkg !== "webcam") return;
    this.frameSeq = frame.seq;
    this.paint(frame.jpeg, frame.width, frame.height);
  }

  // ---------------------------------------------------------------- WebRTC

  private sameCameraNet(candidate: string, phoneIp: string | undefined) {
    if (!phoneIp || phoneIp.includes(":")) return true;
    const address = (candidate || "").split(" ")[4] || "";
    const prefix = phoneIp.split(".").slice(0, 3).join(".");
    return address.startsWith(`${prefix}.`);
  }

  private queueLocal(peer: Peer, item: Peer["queue"][number]) {
    peer.queue.push(item);
    if (peer.answered) void this.drainLocal(peer);
  }

  private async drainLocal(peer: Peer) {
    if (peer.draining) return;
    peer.draining = true;
    while (peer.queue.length && this.peers.get(peer.sessionId) === peer) {
      const item = peer.queue.shift()!;
      try {
        await api.webrtcIce(item);
      } catch {
        /* The next address is sent on its own. */
      }
    }
    peer.draining = false;
  }

  private async addPhoneCandidate(pc: RTCPeerConnection, candidate: RTCIceCandidateInit) {
    const text = (candidate.candidate || "").trim().replace(/^a=/, "");
    if (!text) return false;
    const attempts: RTCIceCandidateInit[] = [
      { candidate: text, sdpMid: candidate.sdpMid || undefined, sdpMLineIndex: candidate.sdpMLineIndex ?? 0 },
      { candidate: text, sdpMLineIndex: candidate.sdpMLineIndex ?? 0 },
    ];
    for (const item of attempts) {
      try {
        await pc.addIceCandidate(item);
        return true;
      } catch {
        /* The next shape drops a mid the phone's offer does not use. */
      }
    }
    return false;
  }

  private offerForWebKit(sdp: string) {
    const codecs: string[] = [];
    for (const line of sdp.replace(/\r\n/g, "\n").split("\n")) {
      const map = line.match(/^a=rtpmap:\d+ ([^/ ]+)\//i);
      if (map && map[1].toUpperCase() !== "RTX") codecs.push(map[1].toUpperCase());
    }
    const onlyH264 = codecs.length > 0 && codecs.every((name) => name === "H264");
    if (onlyH264 && /profile-level-id=420033/i.test(sdp)) return webkitH264(sdp);
    return sdp;
  }

  private connectingText() {
    return this.kind === "webcam" ? "Connecting to the phone camera…" : "Connecting to the phone…";
  }

  private async playDecoded(signal: RtcSignal) {
    this.setStatus(this.connectingText());
    try {
      await api.playDecoded(signal.sessionId, signal.sdp || "");
    } catch {
      this.setStatus("The phone screen did not start.");
    }
  }

  private async openOffer(signal: RtcSignal) {
    if (this.peers.has(signal.sessionId)) return;
    const pc = new RTCPeerConnection({ iceServers: [], bundlePolicy: "max-bundle" });
    const peer: Peer = {
      pc,
      sessionId: signal.sessionId,
      lanIp: signal.lanIp || this.latest?.lanIp || "",
      offerMid: "",
      answered: false,
      draining: false,
      queue: [],
    };
    this.peers.set(signal.sessionId, peer);
    pc.ontrack = (event) => {
      const stream = event.streams[0] ?? new MediaStream([event.track]);
      this.video.muted = true;
      this.video.srcObject = stream;
      this.video.play().catch(() => this.setStatus("Click the window to show the picture."));
    };
    pc.onicecandidate = (event) => {
      if (!event.candidate) return;
      const candidate = localCandidate(event.candidate.candidate, peer.lanIp);
      if (!candidate) return;
      if (this.pkg === "webcam" && !this.sameCameraNet(candidate, signal.phoneIp)) return;
      this.queueLocal(peer, {
        sessionId: signal.sessionId,
        candidate,
        label: event.candidate.sdpMLineIndex ?? 0,
        mid: event.candidate.sdpMid || peer.offerMid || "0",
      });
    };
    const rawOffer = rewriteLocals(signal.sdp || "", signal.phoneIp);
    const profile = h264Profile(rawOffer);
    const remote = this.offerForWebKit(rawOffer);
    peer.offerMid = offerVideoMid(remote);
    try {
      await pc.setRemoteDescription({ type: "offer", sdp: remote });
      const answer = await pc.createAnswer();
      await pc.setLocalDescription(answer);
      let outgoing = rewriteLocals(pc.localDescription?.sdp || answer.sdp || "", peer.lanIp);
      if (remote !== rawOffer && profile) outgoing = wireProfile(outgoing, profile);
      if (/^m=video 0 /m.test(outgoing)) throw new Error("rejected");
      await api.webrtcAnswer(signal.sessionId, outgoing);
    } catch {
      pc.close();
      this.peers.delete(signal.sessionId);
      await this.playDecoded(signal);
      return;
    }
    peer.answered = true;
    await this.drainLocal(peer);
    for (const candidate of this.earlyIce.get(signal.sessionId) ?? []) {
      await this.addPhoneCandidate(pc, candidate);
    }
    this.earlyIce.delete(signal.sessionId);
    this.setStatus(this.kind === "webcam" ? "Connected. Waiting for the camera picture…" : "Connected. Waiting for the picture…");
  }

  private onRtc(signal: RtcSignal) {
    if (!signal || signal.pkg !== this.pkg) return;
    if (signal.kind === "closed") {
      const peer = this.peers.get(signal.sessionId);
      if (peer) {
        peer.pc.close();
        this.peers.delete(signal.sessionId);
      }
      return;
    }
    if (signal.kind === "ice") {
      const candidate: RTCIceCandidateInit = {
        candidate: phoneCandidate(signal.candidate, signal.phoneIp),
        sdpMLineIndex: signal.label ?? 0,
        sdpMid: signal.mid || "0",
      };
      const peer = this.peers.get(signal.sessionId);
      if (!peer || !peer.pc.remoteDescription) {
        const list = this.earlyIce.get(signal.sessionId) ?? [];
        list.push(candidate);
        this.earlyIce.set(signal.sessionId, list);
        return;
      }
      void this.addPhoneCandidate(peer.pc, candidate);
      return;
    }
    if (signal.kind !== "offer" || signal.quiet) return;
    this.setStatus(this.connectingText());
    this.openOffer(signal).catch(() => {
      this.setStatus(this.kind === "webcam" ? "The phone camera did not start." : "The phone screen did not start.");
    });
  }

  // ----------------------------------------------------------------- input

  private point(event: PointerEvent): [number, number] {
    const surface: HTMLElement = this.video.srcObject ? this.video : this.image.naturalWidth ? this.image : this.well!;
    const rect = surface.getBoundingClientRect();
    const boxW = rect.width || 1;
    const boxH = rect.height || 1;
    let x = event.clientX - rect.left;
    let y = event.clientY - rect.top;
    const srcW = this.video.srcObject ? this.video.videoWidth : this.frameWidth;
    const srcH = this.video.srcObject ? this.video.videoHeight : this.frameHeight;
    if (srcW && srcH) {
      const scale = Math.min(boxW / srcW, boxH / srcH);
      const drawnW = srcW * scale;
      const drawnH = srcH * scale;
      x = ((x - (boxW - drawnW) / 2) / drawnW) * srcW;
      y = ((y - (boxH - drawnH) / 2) / drawnH) * srcH;
    }
    return [Math.round(x), Math.round(y)];
  }

  private place(event: PointerEvent): [number, number] {
    const srcW = this.video.videoWidth || this.frameWidth || 1;
    const srcH = this.video.videoHeight || this.frameHeight || 1;
    let [x, y] = this.point(event);
    x = Math.max(0, Math.min(srcW - 1, x));
    y = Math.max(0, Math.min(srcH - 1, y));
    return [x, y];
  }

  private async send(kind: InputKind, extra: Omit<Queued, "kind">) {
    const session = (this.latest?.sessions ?? []).find((s) => this.mine(s));
    const displayId = session?.displayId ?? -1;
    if (displayId < 0 || this.pkg === "webcam") {
      if (displayId < 0 && this.pkg !== "webcam") this.setStatus("The phone has not named this screen yet.");
      return;
    }
    await api.sendInput({
      displayId,
      kind,
      x: extra.x ?? 0,
      y: extra.y ?? 0,
      key: extra.key ?? 0,
      down: Boolean(extra.down),
      flags: extra.flags ?? 0,
      text: "",
    });
  }

  private fingerScreen() {
    if (["webcam", "mirror", "desktop", "mobile"].includes(this.pkg)) return false;
    const session = (this.latest?.sessions ?? []).find((s) => this.mine(s));
    return Boolean(session?.supportTouch);
  }

  private queueInput(kind: InputKind, extra: Omit<Queued, "kind">) {
    const sliding = kind === "move" || (kind === "touch" && extra.key === 2);
    if (sliding) {
      const last = this.gesture[this.gesture.length - 1];
      const sameSlide = last && last.kind === kind && (kind === "move" || last.key === 2);
      if (sameSlide) {
        last.x = extra.x;
        last.y = extra.y;
        void this.pumpGesture();
        return;
      }
    }
    this.gesture.push({ kind, ...extra });
    void this.pumpGesture();
  }

  private async pumpGesture() {
    if (this.sendingGesture) return;
    this.sendingGesture = true;
    try {
      while (this.gesture.length) {
        const item = this.gesture.shift()!;
        await this.send(item.kind, item);
      }
    } finally {
      this.sendingGesture = false;
      if (this.gesture.length) void this.pumpGesture();
    }
  }

  private press = (event: PointerEvent) => {
    this.video.play().catch(() => {});
    if (this.pkg === "webcam" || event.button > 2) return;
    if ((event.target as Element | null)?.closest?.("button")) return;
    event.preventDefault();
    try {
      this.well?.setPointerCapture(event.pointerId);
    } catch {
      /* The moves still arrive on the window. */
    }
    const [x, y] = this.place(event);
    // A mouse button held down is only a click on an app screen. A hold and a
    // swipe are touch: 1 down, 2 move, 0 up. The phone names that in supportTouch.
    if (this.fingerScreen()) {
      this.holding = { finger: true };
      this.queueInput("touch", { key: 1, x, y });
      return;
    }
    this.holding = { finger: false, key: phoneButton(event.button) };
    this.queueInput("move", { x, y });
    this.queueInput("button", { key: this.holding.key, x, y, down: true });
  };

  private drag = (event: PointerEvent) => {
    if (this.pkg === "webcam" || !this.holding) return;
    const [x, y] = this.place(event);
    if (this.holding.finger) this.queueInput("touch", { key: 2, x, y });
    else this.queueInput("move", { x, y });
  };

  private release = (event: PointerEvent) => {
    if (this.pkg === "webcam" || !this.holding) return;
    const [x, y] = this.place(event);
    const mode = this.holding;
    this.holding = null;
    if (mode.finger) {
      this.queueInput("touch", { key: 0, x, y });
      return;
    }
    this.queueInput("move", { x, y });
    this.queueInput("button", { key: mode.key, x, y, down: false });
  };

  pressAndroidKey(code: number) {
    void this.send("key", { key: code, down: true, flags: ANDROID_FLAG });
    void this.send("key", { key: code, down: false, flags: ANDROID_FLAG });
  }

  flipCamera() {
    api.webcamFlip().catch(() => {});
  }

  // ---------------------------------------------------------- camera sink

  // The preview stays on the video element. A second canvas, never inserted
  // into the page, copies frames for OBS Virtual Camera.
  private publishCamera() {
    if (this.pkg !== "webcam") return;
    const canvas = document.createElement("canvas");
    const ctx = canvas.getContext("2d", { alpha: false });
    if (!ctx) return;
    const video = this.video;
    let busy = false;
    let last = 0;
    const grab = () => {
      const width = video.videoWidth;
      const height = video.videoHeight;
      const now = performance.now();
      if (!width || !height || busy || now - last < 50) return;
      last = now;
      const scale = Math.min(1, 1280 / width, 720 / height);
      const cw = Math.max(2, Math.round((width * scale) / 2) * 2);
      const ch = Math.max(2, Math.round((height * scale) / 2) * 2);
      if (canvas.width !== cw) canvas.width = cw;
      if (canvas.height !== ch) canvas.height = ch;
      ctx.drawImage(video, 0, 0, cw, ch);
      busy = true;
      canvas.toBlob((blob) => {
        busy = false;
        if (!blob) return;
        fetch(CAMERA_SINK, { method: "POST", body: blob, keepalive: true }).catch(() => {});
      }, "image/jpeg", 0.7);
    };
    const pump = () => {
      grab();
      if (video.requestVideoFrameCallback) video.requestVideoFrameCallback(pump);
      else setTimeout(pump, 50);
    };
    if (video.requestVideoFrameCallback) video.requestVideoFrameCallback(pump);
    else pump();
  }
}
