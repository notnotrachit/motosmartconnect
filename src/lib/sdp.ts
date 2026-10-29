// SDP helpers shared by the main window and the stream window.
//
// The phone offers H.265, VP8, VP9 and AV1 before H.264. WebKit only answers
// H.264 for a narrow set of profiles, so these helpers adjust the offer that
// WebKit sees and the answer that goes back on the wire.

function linesOf(sdp: string): string[] {
  return (sdp || "")
    .replace(/\r\n/g, "\n")
    .replace(/\r/g, "\n")
    .split("\n")
    .filter((line) => line.length > 0);
}

function fmtpValue(params: string, key: string): string | null {
  if (!params) return null;
  for (const part of params.split(";")) {
    const split = part.trim().split("=");
    if (split[0] && split[0].toLowerCase() === key) return (split.slice(1).join("=") || "").trim();
  }
  return null;
}

function h264Payloads(lines: string[]): Set<string> {
  const h264 = new Set<string>();
  for (const line of lines) {
    const map = line.match(/^a=rtpmap:(\d+) ([^/ ]+)\//i);
    if (map && map[2].toUpperCase() === "H264") h264.add(map[1]);
  }
  return h264;
}

function rewriteH264Fmtp(sdp: string, fmtpBody: string): string {
  const lines = linesOf(sdp);
  const h264 = h264Payloads(lines);
  const out = lines.map((line) => {
    const fmtp = line.match(/^a=fmtp:(\d+) (.*)$/i);
    if (!fmtp || !h264.has(fmtp[1]) || fmtpValue(fmtp[2], "apt")) return line;
    return `a=fmtp:${fmtp[1]} ${fmtpBody}`;
  });
  return `${out.join("\r\n")}\r\n`;
}

// WebKit answers H.264 only for constrained baseline level 3.1, mode 1.
// This phone offers baseline level 5.1 (420033). Left as-is, the video line is
// rejected and ICE stays "new", so the phone never receives an address.
const WEBKIT_H264 = "level-asymmetry-allowed=1;packetization-mode=1;profile-level-id=42e01f";

export function webkitH264(sdp: string): string {
  return rewriteH264Fmtp(sdp, WEBKIT_H264);
}

// The phone offered this profile. WebKit still answers against 42e01f locally,
// but the copy on the wire must name the offered profile or the phone keeps its
// addresses and never sends them.
export function h264Profile(sdp: string): string {
  const lines = linesOf(sdp);
  const h264 = h264Payloads(lines);
  for (const line of lines) {
    const fmtp = line.match(/^a=fmtp:(\d+) (.*)$/i);
    if (!fmtp || !h264.has(fmtp[1]) || fmtpValue(fmtp[2], "apt")) continue;
    const profile = fmtpValue(fmtp[2], "profile-level-id");
    if (profile && /^[0-9a-fA-F]{6}$/.test(profile)) return profile.toLowerCase();
  }
  return "";
}

export function wireProfile(sdp: string, profile: string): string {
  if (!profile || !/^[0-9a-fA-F]{6}$/.test(profile)) return sdp;
  const id = profile.toLowerCase();
  return rewriteH264Fmtp(sdp, `level-asymmetry-allowed=1;packetization-mode=1;profile-level-id=${id}`);
}

/** The mid of the offer's video section, used when a local candidate has none. */
export function offerVideoMid(sdp: string): string {
  let inVideo = false;
  for (const line of (sdp || "").replace(/\r\n/g, "\n").split("\n")) {
    if (line.startsWith("m=")) inVideo = line.startsWith("m=video ");
    else if (inVideo && line.startsWith("a=mid:")) return line.slice("a=mid:".length).trim();
  }
  return "";
}

export function rewriteLocals(text: string, ip: string | undefined): string {
  if (!text || !ip) return text || "";
  return text.replace(/\S+\.local/g, ip);
}

function privateV4(address: string): boolean {
  const match = /^(\d+)\.(\d+)\.(\d+)\.(\d+)$/.exec(address || "");
  if (!match) return false;
  const a = Number(match[1]);
  const b = Number(match[2]);
  if (a === 10 || (a === 192 && b === 168)) return true;
  return a === 172 && b >= 16 && b <= 31;
}

/** A host UDP candidate on a private LAN address, with mDNS names replaced, or null. */
export function localCandidate(candidate: string, ip: string): string | null {
  if (!candidate || !/ typ host/i.test(candidate) || !/ udp /i.test(candidate)) return null;
  const text = rewriteLocals(candidate, ip);
  if (text.includes(".local")) return null;
  const address = text.split(" ")[4];
  if (ip && address === ip) return text;
  if (privateV4(address)) return text;
  return null;
}

export function phoneCandidate(candidate: string | undefined, ip: string | undefined): string {
  if (!candidate) return "";
  return rewriteLocals(candidate, ip);
}

export function normalizeCandidate(text: string | undefined): string {
  const trimmed = (text || "").trim();
  return trimmed.startsWith("a=") ? trimmed.slice(2) : trimmed;
}
