import { EXTENDED, EXTENDED_FLAG, SCANCODES, ANDROID_FLAG, phoneButton } from "@/lib/keymap";
import { h264Profile, localCandidate, normalizeCandidate, offerVideoMid, phoneCandidate, rewriteLocals } from "@/lib/sdp";
import { api, errorText } from "@/lib/tauri";
import type { InputKind, MirrorView, RtcSignal, Snapshot } from "@/lib/types";
import { stripLogTime } from "@/lib/format";

/**
 * Owns the phone-screen picture in the main window: the WebRTC answer, the
 * JPEG fallback frames, picture diagnostics and pointer/keyboard forwarding.
 *
 * It is deliberately framework-agnostic. The <video>/<img> elements are created
 * here and survive React re-renders; components mount them into a container and
 * subscribe to {@link ScreenState} for everything they draw around the picture.
 */

export const SCREEN_IDLE = "The phone screen shows here.";
export const SCREEN_OFFLINE = "Open Smart Connect on the phone to reconnect.";
export const SCREEN_ASKING = "Asking the phone to share its screen…";

const MIRROR_STATUS: Record<string, string> = {
  answering: "Answering the phone screen on this Mac.",
  connecting: "Connecting the screen to the phone.",
  connected: "The screen link is up. Waiting for a picture.",
  video: "Video is arriving. The picture has not decoded yet.",
  failed: "The screen link failed. Both devices need the same Wi-Fi.",
  closed: "The phone stopped the screen.",
};

export interface ScreenState {
  /** Status line for the screen well. Equal to SCREEN_IDLE / SCREEN_OFFLINE when nothing is happening. */
  label: string;
  /** A picture (video or JPEG frame) is on screen. */
  live: boolean;
  width: number;
  height: number;
  activeSession: string;
  /** The active session has a display id, so keys and pointer input reach the phone. */
  canInput: boolean;
}

interface IceItem {
  sessionId: string;
  candidate: string;
  label: number;
  mid: string;
}

interface Peer {
  pc: RTCPeerConnection;
  stream: MediaStream | null;
  displayId: number;
  quiet: boolean;
  sessionId: string;
  lanIp: string;
  localSeen: number;
  localSent: number;
  localDropped: number;
  remoteAdded: number;
  remoteRejected: number;
  gathered: boolean;
  rejectedVideo: boolean;
  wireProfile: string;
  offerMid: string;
  answered: boolean;
  answeredAt: number;
  rawSdp: string;
  decoder: boolean;
  draining: boolean;
  nudged: boolean;
  queue: IceItem[];
}

type Target = "screen" | "cross";

interface InputExtra {
  kind: InputKind;
  x?: number;
  y?: number;
  key?: number;
  down?: boolean;
  flags?: number;
  text?: string;
}

type Listener = (state: ScreenState) => void;

export class MirrorController {
  readonly video: HTMLVideoElement;
  readonly image: HTMLImageElement;
  private readonly quietVideo: HTMLVideoElement;
  private well: HTMLElement | null = null;
  /** Picture box inside the well (inside the bezel); pointer mapping is relative to it. */
  private host: HTMLElement | null = null;
  private pad: HTMLElement | null = null;

  private latest: Snapshot | null = null;
  private frameSeq = 0;
  private frameLive = false;
  private frameWidth = 0;
  private frameHeight = 0;
  private activeSession = "";
  private readonly peers = new Map<string, Peer>();
  private readonly earlyIce = new Map<string, RTCIceCandidateInit[]>();
  private moveAt = 0;
  private readonly held = new Set<number>();
  private pictureTimer = 0;
  private readingStats = false;
  private polling = 0;

  private state: ScreenState = {
    label: SCREEN_IDLE,
    live: false,
    width: 0,
    height: 0,
    activeSession: "",
    canInput: false,
  };
  private readonly listeners = new Set<Listener>();

  constructor(private readonly reportError: (message: string) => void) {
    this.video = document.createElement("video");
    this.video.autoplay = true;
    this.video.playsInline = true;
    this.video.muted = true;
    this.video.className = "screen-media z-[1]";
    this.video.addEventListener("resize", () => {
      if (this.video.videoWidth > 0) {
        this.setLive(true);
        this.stopPictureWatch();
      }
      this.emit();
    });

    this.image = document.createElement("img");
    this.image.alt = "";
    this.image.hidden = true;
    this.image.decoding = "async";
    this.image.className = "screen-media z-[2]";

    // Quiet sessions (cross control) still need a playing sink, but are never shown.
    this.quietVideo = document.createElement("video");
    this.quietVideo.autoplay = true;
    this.quietVideo.playsInline = true;
    this.quietVideo.muted = true;
    this.quietVideo.setAttribute("aria-hidden", "true");
    Object.assign(this.quietVideo.style, {
      position: "absolute",
      width: "1px",
      height: "1px",
      opacity: "0",
      pointerEvents: "none",
    });
    document.body.append(this.quietVideo);
  }

  // ---------------------------------------------------------------- state

  getState = (): ScreenState => this.state;

  subscribe = (fn: Listener): (() => void) => {
    this.listeners.add(fn);
    return () => this.listeners.delete(fn);
  };

  private emit(patch: Partial<ScreenState> = {}) {
    const width = this.frameWidth || this.video.videoWidth;
    const height = this.frameHeight || this.video.videoHeight;
    const next: ScreenState = {
      ...this.state,
      ...patch,
      width,
      height,
      activeSession: this.activeSession,
      canInput: Boolean(this.activeSession) && this.displayFor(this.activeSession) >= 0,
    };
    const s = this.state;
    if (
      s.label === next.label && s.live === next.live && s.width === next.width && s.height === next.height
      && s.activeSession === next.activeSession && s.canInput === next.canInput
    ) return;
    this.state = next;
    for (const fn of this.listeners) fn(next);
  }

  private setLabel(label: string) {
    this.emit({ label });
  }

  private setLive(live: boolean) {
    this.emit({ live });
  }

  private hasPicture() {
    return this.frameLive || this.video.videoWidth > 0;
  }

  // ------------------------------------------------------------- mounting

  /**
   * Put the picture elements into `host` (a React-free child of the well) and
   * wire pointer/keyboard input on `well`.
   */
  mountScreen(well: HTMLElement, host: HTMLElement): () => void {
    this.well = well;
    this.host = host;
    host.append(this.video, this.image);
    this.video.play().catch(() => {});
    const unbindPointer = this.bindPointer(well);
    const unbindKeys = this.bindKeys(well);
    const prime = () => this.primeVideo();
    well.addEventListener("click", prime);
    return () => {
      unbindPointer();
      unbindKeys();
      well.removeEventListener("click", prime);
      if (this.well === well) {
        this.well = null;
        this.host = null;
      }
    };
  }

  /** Wire the cross-control pad. */
  mountPad(pad: HTMLElement): () => void {
    this.pad = pad;
    const unbindPointer = this.bindPointer(pad);
    const unbindKeys = this.bindKeys(pad);
    const enter = () => void api.crossShow(true).catch(() => {});
    const leave = () => void this.releaseHeld("cross").then(() => api.crossShow(false)).catch(() => {});
    pad.addEventListener("pointerenter", enter);
    pad.addEventListener("pointerleave", leave);
    return () => {
      unbindPointer();
      unbindKeys();
      pad.removeEventListener("pointerenter", enter);
      pad.removeEventListener("pointerleave", leave);
      if (this.pad === pad) this.pad = null;
    };
  }

  /** Start polling for JPEG fallback frames. Safe to call more than once. */
  start() {
    if (this.polling) return;
    this.polling = window.setInterval(() => void this.pullFrame(), 200);
  }

  // ------------------------------------------------------------ snapshot

  onSnapshot(snapshot: Snapshot) {
    this.latest = snapshot;
    const phones = snapshot.phones ?? [];
    const paired = phones.length > 0;
    const live = phones.some((phone) => phone.connected);
    const idleCopy = this.state.label === SCREEN_IDLE || this.state.label === SCREEN_OFFLINE;
    if (paired && idleCopy && this.video.videoWidth === 0 && !this.frameLive) {
      this.setLabel(live ? SCREEN_IDLE : SCREEN_OFFLINE);
    }

    const stream = snapshot.stream;
    if (stream && !stream.quiet) this.activeSession = stream.sessionId;
    for (const session of snapshot.sessions ?? []) {
      const peer = this.peers.get(session.sessionId);
      if (peer && session.displayId >= 0) peer.displayId = session.displayId;
    }
    this.followScreenLog(snapshot);
    this.emit();
  }

  private displayFor(sessionId: string): number {
    const peer = this.peers.get(sessionId);
    if (peer && peer.displayId >= 0) return peer.displayId;
    const session = (this.latest?.sessions ?? []).find((item) => item.sessionId === sessionId);
    return session?.displayId ?? -1;
  }

  private paintsOnDesk(sessionId: string): boolean {
    if (!sessionId) return true;
    const session = (this.latest?.sessions ?? []).find((item) => item.sessionId === sessionId);
    if (!session) return true;
    return session.pkg === "mirror" || !session.pkg;
  }

  // --------------------------------------------------------------- input

  async sendInput(target: Target, extra: InputExtra) {
    const displayId = target === "cross" ? 0 : this.displayFor(this.activeSession);
    if (displayId < 0 && target !== "cross") return;
    await api.sendInput({
      displayId,
      kind: extra.kind,
      x: extra.x ?? 0,
      y: extra.y ?? 0,
      key: extra.key ?? 0,
      down: Boolean(extra.down),
      flags: extra.flags ?? 0,
      text: extra.text ?? "",
    });
  }

  async pressAndroidKey(code: number) {
    await this.sendInput("screen", { kind: "key", key: code, down: true, flags: ANDROID_FLAG });
    await this.sendInput("screen", { kind: "key", key: code, down: false, flags: ANDROID_FLAG });
  }

  async typeText(text: string) {
    if (!text) return;
    await this.sendInput("screen", { kind: "text", text });
  }

  /** Release any mouse buttons still held on the phone. */
  async releaseHeld(target: Target) {
    for (const button of this.held) {
      await this.sendInput(target, { kind: "button", key: button, down: false });
    }
    this.held.clear();
  }

  private framePoint(event: { clientX: number; clientY: number }, element: HTMLElement, width: number, height: number) {
    const rect = element.getBoundingClientRect();
    const boxW = rect.width || 1;
    const boxH = rect.height || 1;
    let x = event.clientX - rect.left;
    let y = event.clientY - rect.top;
    const video = this.video;
    const intrinsicW = element === this.host ? this.frameWidth || video.videoWidth : element === video ? video.videoWidth : 0;
    const intrinsicH = element === this.host ? this.frameHeight || video.videoHeight : element === video ? video.videoHeight : 0;
    if (intrinsicW && intrinsicH) {
      const scale = Math.min(boxW / intrinsicW, boxH / intrinsicH);
      const drawnW = intrinsicW * scale;
      const drawnH = intrinsicH * scale;
      const left = (boxW - drawnW) / 2;
      const top = (boxH - drawnH) / 2;
      x = ((x - left) / drawnW) * intrinsicW;
      y = ((y - top) / drawnH) * intrinsicH;
      return [Math.round(x), Math.round(y)] as const;
    }
    return [Math.round((x / boxW) * width), Math.round((y / boxH) * height)] as const;
  }

  private pointerSession(event: { clientX: number; clientY: number }, element: HTMLElement) {
    if (element === this.pad) {
      const cross = this.latest?.cross;
      if (!cross?.active) return null;
      const [x, y] = this.framePoint(event, element, cross.width || 1, cross.height || 1);
      return { target: "cross" as Target, x, y };
    }
    if (!this.activeSession) return null;
    const width = this.frameWidth || this.video.videoWidth || 1;
    const height = this.frameHeight || this.video.videoHeight || 1;
    const surface = this.frameWidth && this.host ? this.host : this.video;
    const [x, y] = this.framePoint(event, surface, width, height);
    if (x < 0 || y < 0 || (width && x > width) || (height && y > height)) return null;
    return { target: "screen" as Target, x, y };
  }

  private bindPointer(element: HTMLElement): () => void {
    // Overlay buttons inside the well must keep their own clicks.
    const fromControl = (event: Event) => Boolean((event.target as Element | null)?.closest?.("button, input, a"));
    const down = (event: PointerEvent) => {
      if (fromControl(event)) return;
      const point = this.pointerSession(event, element);
      if (!point) return;
      element.setPointerCapture?.(event.pointerId);
      element.focus?.();
      const button = phoneButton(event.button);
      this.held.add(button);
      void this.sendInput(point.target, { kind: "button", x: point.x, y: point.y, key: button, down: true });
    };
    const up = (event: PointerEvent) => {
      if (fromControl(event)) return;
      const point = this.pointerSession(event, element);
      if (!point) return;
      const button = phoneButton(event.button);
      this.held.delete(button);
      void this.sendInput(point.target, { kind: "button", x: point.x, y: point.y, key: button, down: false });
    };
    const move = (event: PointerEvent) => {
      const now = performance.now();
      if (now - this.moveAt < 30) return;
      this.moveAt = now;
      const point = this.pointerSession(event, element);
      if (!point) return;
      void this.sendInput(point.target, { kind: "move", x: point.x, y: point.y });
    };
    const wheel = (event: WheelEvent) => {
      const point = this.pointerSession(event, element);
      if (!point) return;
      event.preventDefault();
      void this.sendInput(point.target, { kind: "wheel", x: point.x, y: point.y, key: 4, down: event.deltaY > 0 });
    };
    const menu = (event: Event) => event.preventDefault();
    element.addEventListener("pointerdown", down);
    element.addEventListener("pointerup", up);
    element.addEventListener("pointermove", move);
    element.addEventListener("wheel", wheel, { passive: false });
    element.addEventListener("contextmenu", menu);
    return () => {
      element.removeEventListener("pointerdown", down);
      element.removeEventListener("pointerup", up);
      element.removeEventListener("pointermove", move);
      element.removeEventListener("wheel", wheel);
      element.removeEventListener("contextmenu", menu);
    };
  }

  private bindKeys(element: HTMLElement): () => void {
    const target: Target = element === this.pad ? "cross" : "screen";
    const skip = (event: KeyboardEvent) => Boolean((event.target as Element | null)?.matches?.("input, textarea, button"));
    const down = (event: KeyboardEvent) => {
      if (skip(event)) return;
      const extended = EXTENDED[event.code];
      const code = SCANCODES[event.code];
      if (extended == null && code == null) return;
      event.preventDefault();
      if (target === "cross" && !this.latest?.cross?.active) return;
      void this.sendInput(target, { kind: "key", key: extended ?? code, down: true, flags: extended == null ? 0 : EXTENDED_FLAG });
    };
    const up = (event: KeyboardEvent) => {
      if (skip(event)) return;
      const extended = EXTENDED[event.code];
      const code = SCANCODES[event.code];
      if (extended == null && code == null) return;
      event.preventDefault();
      void this.sendInput(target, { kind: "key", key: extended ?? code, down: false, flags: extended == null ? 0 : EXTENDED_FLAG });
    };
    element.addEventListener("keydown", down);
    element.addEventListener("keyup", up);
    return () => {
      element.removeEventListener("keydown", down);
      element.removeEventListener("keyup", up);
    };
  }

  // ------------------------------------------------------------- picture

  primeVideo() {
    this.video.muted = true;
    this.video.play().catch(() => {});
  }

  /** Called right before asking the phone to mirror. */
  beginMirror() {
    this.primeVideo();
    this.setScreenStatus(SCREEN_ASKING);
  }

  /** Called when asking the phone failed for a reason the user should retry. */
  resetIdle() {
    this.setScreenStatus(SCREEN_IDLE);
  }

  get currentSession() {
    return this.activeSession;
  }

  setScreenStatus(text: string) {
    if (this.hasPicture()) return;
    this.emit({ label: text, live: false });
  }

  private paintFrame(jpeg: string, width: number, height: number) {
    if (!jpeg) return;
    this.frameWidth = width || this.frameWidth;
    this.frameHeight = height || this.frameHeight;
    this.image.hidden = false;
    this.image.src = `data:image/jpeg;base64,${jpeg}`;
    this.frameLive = true;
    this.setLive(true);
  }

  onFrame(frame: MirrorView) {
    if (!frame?.jpeg || frame.seq === this.frameSeq) return;
    this.frameSeq = frame.seq;
    if (!this.paintsOnDesk(frame.sessionId)) return;
    this.paintFrame(frame.jpeg, frame.width, frame.height);
  }

  private async pullFrame() {
    let frame: MirrorView;
    try {
      frame = await api.mirrorFrame(this.frameSeq);
    } catch {
      return;
    }
    if (!frame) return;
    if (frame.jpeg && !this.paintsOnDesk(frame.sessionId)) {
      this.frameSeq = frame.seq;
      return;
    }
    if (frame.jpeg) {
      this.frameSeq = frame.seq;
      this.paintFrame(frame.jpeg, frame.width, frame.height);
    } else if (frame.seq !== this.frameSeq && this.paintsOnDesk(frame.sessionId)) {
      this.frameSeq = frame.seq;
      this.frameLive = false;
      this.frameWidth = 0;
      this.frameHeight = 0;
      this.image.hidden = true;
      this.image.removeAttribute("src");
      this.setLive(this.video.videoWidth > 0);
    }
    if (!this.frameLive && MIRROR_STATUS[frame.state]) this.setScreenStatus(MIRROR_STATUS[frame.state]);
  }

  private stopPictureWatch() {
    if (this.pictureTimer) {
      clearInterval(this.pictureTimer);
      this.pictureTimer = 0;
    }
  }

  private pictureStatus(pc: RTCPeerConnection, peer: Peer, bytes: number, frames: number, mime: string): string {
    const link = pc.connectionState || "new";
    const ice = pc.iceConnectionState || "new";
    const gather = pc.iceGatheringState || "new";
    const codec = mime ? `, ${mime}` : "";
    const counts = `${Math.round(bytes / 1024)} KB in, ${frames} frames${codec}`;
    const path = `${peer.localSent || 0} from this Mac, ${peer.remoteAdded || 0} from the phone`;
    if (link === "failed" || ice === "failed") return `The picture link failed. ${path}. ${counts}.`;
    if (bytes > 0 && frames === 0) return `Video is arriving, but no frame has decoded. ${counts}. Link ${link}.`;
    if (frames > 0) return `A frame decoded, but the picture is not on screen yet. ${counts}.`;
    const profile = peer.wireProfile ? ` Answered as H264 ${peer.wireProfile}.` : "";
    const rejected = peer.remoteRejected ? ` ${peer.remoteRejected} from the phone could not be used.` : "";
    if (peer.rejectedVideo) return `This Mac rejected the phone's video format. Gathering ${gather}. ${path}.`;
    if (!peer.lanIp) return `This Mac has no LAN address to offer the phone. ${path}.`;
    if (gather === "complete" && !peer.localSent) return `This Mac found no usable address for the picture. ${peer.localDropped || 0} were hidden. ${path}.`;
    if (peer.remoteRejected && !peer.remoteAdded) return `The phone sent an address this Mac could not use.${profile} ${path}.${rejected} Ice ${ice}.`;
    if (peer.localSent && !peer.remoteAdded) return `Answered.${profile} Waiting for the phone's address. ${path}. Gathering ${gather}. Ice ${ice}.`;
    if (link === "connected") return `Connected. The phone has not sent video yet. ${path}. ${counts}.`;
    return `Waiting for the picture. ${path}. Gathering ${gather}, seen ${peer.localSeen || 0}, hidden ${peer.localDropped || 0}. Link ${link}, ice ${ice}. ${counts}.`;
  }

  private followScreenLog(snapshot: Snapshot) {
    if (this.pictureTimer || this.video.videoWidth > 0) return;
    const last = snapshot.log?.at(-1);
    if (!last) return;
    const text = stripLogTime(last);
    if (!this.state.label.startsWith("Asking the phone")) return;
    if (text.startsWith("Asking the phone")) return;
    if (/screen|picture|encoder|declined|share|preparing|Answered|opened|H264|H265|format|address/.test(text)) {
      this.setScreenStatus(text);
    }
  }

  private nudgeDecoder(peer: Peer) {
    if (peer.nudged) return;
    const ice = peer.pc.iceConnectionState;
    const ready = peer.pc.connectionState === "connected" || ice === "connected" || ice === "completed";
    if (!ready) return;
    const tracks = peer.stream?.getVideoTracks?.() ?? [];
    if (!tracks.length) return;
    peer.nudged = true;
    for (const track of tracks) {
      track.enabled = false;
      track.enabled = true;
    }
  }

  private watchPicture(pc: RTCPeerConnection, sessionId: string) {
    this.stopPictureWatch();
    const tick = async () => {
      const peer = this.peers.get(sessionId);
      if (!peer || peer.pc !== pc || peer.quiet) {
        this.stopPictureWatch();
        return;
      }
      if (this.video.videoWidth > 0) {
        this.setLive(true);
        this.stopPictureWatch();
        return;
      }
      if (this.readingStats) return;
      this.readingStats = true;
      let bytes = 0;
      let frames = 0;
      let mime = "";
      try {
        const stats = await pc.getStats();
        stats.forEach((report) => {
          if (report.type !== "inbound-rtp") return;
          if (report.kind !== "video" && report.mediaType !== "video") return;
          bytes = report.bytesReceived || 0;
          frames = report.framesDecoded || 0;
          const codec = report.codecId ? stats.get(report.codecId) : null;
          mime = codec?.mimeType || mime;
        });
      } catch {
        this.readingStats = false;
        return;
      }
      this.readingStats = false;
      if (frames > 0 && this.video.videoWidth > 0) {
        this.setLive(true);
        this.stopPictureWatch();
        return;
      }
      // The window accepted the offer but no video arrived. Decode it in Rust instead of staying blank.
      if (!peer.decoder && peer.answeredAt && Date.now() - peer.answeredAt > 6000 && bytes === 0 && frames === 0) {
        peer.decoder = true;
        this.stopPictureWatch();
        pc.close();
        this.peers.delete(sessionId);
        try {
          await api.playDecoded(sessionId, peer.rawSdp);
          this.setScreenStatus("Connecting to the phone…");
        } catch (error) {
          this.setScreenStatus("The phone screen did not start.");
          this.reportError(errorText(error));
        }
        return;
      }
      this.setScreenStatus(this.pictureStatus(pc, peer, bytes, frames, mime));
    };
    this.pictureTimer = window.setInterval(() => void tick(), 1000);
    void tick();
  }

  /** Switch the well to another mirrored session that already has a stream. */
  showSession(sessionId: string) {
    const peer = this.peers.get(sessionId);
    if (!peer?.stream) return;
    this.activeSession = sessionId;
    this.video.muted = true;
    this.video.srcObject = peer.stream;
    if (this.video.videoWidth > 0) this.setLive(true);
    else this.setScreenStatus("Connected. Waiting for the picture…");
    this.video.play().catch(() => this.setScreenStatus("Click the screen to show the picture."));
    this.emit();
  }

  // ---------------------------------------------------------------- WebRTC

  private retirePeers(exceptId: string) {
    this.stopPictureWatch();
    for (const [id, old] of [...this.peers]) {
      if (id === exceptId) continue;
      old.pc.close();
      old.queue = [];
      this.peers.delete(id);
    }
  }

  private queueLocal(peer: Peer, item: IceItem) {
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
        peer.localSent += 1;
      } catch (error) {
        this.reportError(errorText(error));
      }
    }
    peer.draining = false;
    if (peer.queue.length && peer.answered && this.peers.get(peer.sessionId) === peer) void this.drainLocal(peer);
  }

  private async addPhoneCandidate(pc: RTCPeerConnection, candidate: RTCIceCandidateInit): Promise<boolean> {
    const text = normalizeCandidate(candidate.candidate);
    if (!text) return false;
    const mid = pc.getTransceivers().find((item) => item.receiver?.track?.kind === "video")?.mid || "";
    const attempts: RTCIceCandidateInit[] = [{ ...candidate, candidate: text }];
    if (mid && candidate.sdpMid !== mid) {
      attempts.push({ candidate: text, sdpMid: mid, sdpMLineIndex: candidate.sdpMLineIndex ?? 0 });
    }
    attempts.push({ candidate: text, sdpMLineIndex: candidate.sdpMLineIndex ?? 0 });
    for (const item of attempts) {
      try {
        await pc.addIceCandidate(item);
        return true;
      } catch {
        // The next shape drops a mid the phone's offer does not use.
      }
    }
    return false;
  }

  private async openOffer(signal: RtcSignal) {
    this.retirePeers(signal.sessionId);
    const previous = this.peers.get(signal.sessionId);
    if (previous) {
      previous.pc.close();
      previous.queue = [];
    }
    const pc = new RTCPeerConnection({ iceServers: [], bundlePolicy: "max-bundle" });
    const quiet = Boolean(signal.quiet);
    const peer: Peer = {
      pc,
      stream: null,
      displayId: -1,
      quiet,
      sessionId: signal.sessionId,
      lanIp: signal.lanIp || this.latest?.lanIp || "",
      localSeen: 0,
      localSent: 0,
      localDropped: 0,
      remoteAdded: 0,
      remoteRejected: 0,
      gathered: false,
      rejectedVideo: false,
      wireProfile: "",
      offerMid: "",
      answered: false,
      answeredAt: 0,
      rawSdp: signal.sdp || "",
      decoder: false,
      draining: false,
      nudged: false,
      queue: [],
    };
    this.peers.set(signal.sessionId, peer);
    const target = quiet ? this.quietVideo : this.video;
    pc.ontrack = (event) => {
      const stream = event.streams[0] ?? new MediaStream([event.track]);
      peer.stream = stream;
      if (!quiet) {
        this.activeSession = signal.sessionId;
        this.video.muted = true;
        this.video.srcObject = stream;
        this.setScreenStatus("Connected. Waiting for the picture…");
        this.video.play().catch(() => this.setScreenStatus("Click the screen to show the picture."));
        this.nudgeDecoder(peer);
        this.emit();
      } else {
        this.quietVideo.srcObject = stream;
        this.quietVideo.play().catch(() => {});
      }
      target.play?.().catch(() => {});
    };
    pc.oniceconnectionstatechange = () => {
      if (quiet || this.peers.get(signal.sessionId) !== peer) return;
      const state = pc.iceConnectionState;
      this.nudgeDecoder(peer);
      if (state === "failed") this.setScreenStatus("The screen link did not connect. Both devices need the same Wi-Fi.");
      else if (state === "checking") this.setScreenStatus("Connecting to the phone…");
    };
    pc.onconnectionstatechange = () => {
      if (quiet || this.peers.get(signal.sessionId) !== peer) return;
      this.nudgeDecoder(peer);
      if (pc.connectionState === "failed") this.setScreenStatus("The picture link failed after the network connected.");
    };
    pc.onicecandidate = (event) => {
      if (!event.candidate) {
        peer.gathered = true;
        return;
      }
      peer.localSeen += 1;
      const candidate = localCandidate(event.candidate.candidate, peer.lanIp);
      if (!candidate) {
        peer.localDropped += 1;
        return;
      }
      // The phone drops an address that arrives before the answer, then sends none of its own.
      this.queueLocal(peer, {
        sessionId: signal.sessionId,
        candidate,
        label: event.candidate.sdpMLineIndex ?? 0,
        mid: event.candidate.sdpMid || peer.offerMid || "0",
      });
    };
    // Answer the offer the phone sent. Filtering it to H.264 makes WebKit
    // reject the video line. Left whole, WebKit hardware-decodes the codec it accepts.
    const rawOffer = rewriteLocals(signal.sdp || "", signal.phoneIp);
    peer.wireProfile = h264Profile(rawOffer);
    peer.offerMid = offerVideoMid(rawOffer);
    try {
      await pc.setRemoteDescription({ type: "offer", sdp: rawOffer });
      const answer = await pc.createAnswer();
      await pc.setLocalDescription(answer);
      const outgoing = rewriteLocals(pc.localDescription?.sdp || answer.sdp || "", peer.lanIp);
      peer.rejectedVideo = /^m=video 0 /m.test(outgoing);
      if (peer.rejectedVideo) throw new Error("rejected");
      await api.webrtcAnswer(signal.sessionId, outgoing);
    } catch {
      pc.close();
      this.peers.delete(signal.sessionId);
      await api.playDecoded(signal.sessionId, signal.sdp || "");
      if (!quiet) this.setScreenStatus("Connecting to the phone…");
      return;
    }
    peer.answered = true;
    peer.answeredAt = Date.now();
    await this.drainLocal(peer);
    if (!quiet) this.watchPicture(pc, signal.sessionId);
    for (const candidate of this.earlyIce.get(signal.sessionId) ?? []) {
      if (await this.addPhoneCandidate(pc, candidate)) peer.remoteAdded += 1;
      else peer.remoteRejected += 1;
    }
    this.earlyIce.delete(signal.sessionId);
  }

  async onRtc(signal: RtcSignal) {
    if (signal.kind === "closed") {
      const peer = this.peers.get(signal.sessionId);
      if (peer) {
        peer.pc.close();
        this.peers.delete(signal.sessionId);
      }
      if (this.activeSession === signal.sessionId) {
        this.activeSession = "";
        this.video.srcObject = null;
        this.stopPictureWatch();
        this.emit({ live: false, label: SCREEN_IDLE });
      }
      return;
    }
    if (signal.kind === "ice") {
      const pkg = signal.pkg || "";
      if (pkg && pkg !== "mirror" && !signal.quiet && !this.peers.has(signal.sessionId)) return;
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
      if (await this.addPhoneCandidate(peer.pc, candidate)) peer.remoteAdded += 1;
      else peer.remoteRejected += 1;
      return;
    }
    if (signal.kind === "offer") {
      const pkg = signal.pkg || "mirror";
      if (!signal.quiet && pkg !== "mirror") return;
      if (!signal.quiet) this.setScreenStatus("Connecting to the phone…");
      try {
        await this.openOffer(signal);
      } catch (error) {
        const message = errorText(error).replace(/\s+/g, " ").slice(0, 140);
        this.setScreenStatus("The phone screen did not start.");
        this.reportError(`The phone screen did not start. ${message}`);
        api.screenNote(`The phone screen did not start. ${message}`).catch(() => {});
      }
    }
  }
}
