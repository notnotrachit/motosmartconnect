// Shapes serialized by the Rust engine (serde camelCase). Keep in sync with
// src-tauri/src/engine.rs, desk.rs, mirror.rs and nfs.rs.

export interface PhoneView {
  realId: string;
  name: string;
  connected: boolean;
  userDisconnected: boolean;
}

export interface NoticeAction {
  id: number;
  name: string;
}

export interface NoticeView {
  key: string;
  app: string;
  pkg: string;
  title: string;
  content: string;
  canReply: boolean;
  actions: NoticeAction[];
  /** Phone app icon as a data URL, or empty. */
  icon: string;
}

export interface AppView {
  pkg: string;
  name: string;
  cmp: string;
  userId: number;
  iconName: string;
}

export interface FileView {
  name: string;
  path: string;
}

export interface SessionView {
  sessionId: string;
  title: string;
  pkg: string;
  displayId: number;
  width: number;
  height: number;
  quiet: boolean;
  supportTouch: boolean;
}

export interface CrossView {
  active: boolean;
  width: number;
  height: number;
}

export interface ThreadView {
  key: string;
  title: string;
  address: string;
  preview: string;
  date: number;
  unread: number;
}

export interface MessageView {
  id: string;
  outgoing: boolean;
  body: string;
  date: number;
  pending: boolean;
  failed: boolean;
}

export interface SmsView {
  /** waiting, denied, off, loading, or ready. */
  phase: string;
  unread: number;
  threads: ThreadView[];
  openKey: string;
  openTitle: string;
  openAddress: string;
  open: MessageView[];
}

export interface Snapshot {
  deviceName: string;
  realDeviceId: string;
  fingerprintB64: string;
  port: number;
  ips: string[];
  qrSvg: string;
  qrExpiresIn: number;
  pairing: boolean;
  phones: PhoneView[];
  log: string[];
  listening: boolean;
  error: string | null;
  notifications: NoticeView[];
  clipboardReady: boolean;
  apps: AppView[];
  iconsRevision: number;
  files: FileView[];
  sessions: SessionView[];
  stream: SessionView | null;
  cross: CrossView;
  lanIp: string;
  messages: SmsView;
}

export interface IconPack {
  revision: number;
  icons: Record<string, string>;
}

export interface MirrorView {
  seq: number;
  jpeg: string;
  width: number;
  height: number;
  state: string;
  sessionId: string;
}

export interface PhoneEntry {
  name: string;
  directory: boolean;
  size: number;
}

/** Payload of the "rtc" event and of page_offers. */
export interface RtcSignal {
  kind: "offer" | "ice" | "closed" | string;
  sessionId: string;
  pkg?: string;
  quiet?: boolean;
  sdp?: string;
  candidate?: string;
  label?: number;
  mid?: string;
  lanIp?: string;
  phoneIp?: string;
}

export type InputKind = "button" | "move" | "wheel" | "key" | "text" | "touch";

export interface InputArgs {
  displayId: number;
  kind: InputKind;
  x: number;
  y: number;
  key: number;
  down: boolean;
  flags: number;
  text: string;
}
