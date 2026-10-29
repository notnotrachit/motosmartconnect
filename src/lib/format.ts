export interface LogLine {
  time: string;
  text: string;
}

/** Engine log lines look like "12:04:55Z Opened the screen." */
export function parseLog(line: string): LogLine {
  const match = /^(\d\d:\d\d:\d\d)Z\s+(.*)$/.exec(line);
  return match ? { time: match[1], text: match[2] } : { time: "", text: line };
}

export function stripLogTime(line: string): string {
  return line.replace(/^\d\d:\d\d:\d\dZ\s+/, "");
}

export function formatBytes(bytes: number): string {
  if (!bytes) return "";
  const units = ["B", "KB", "MB", "GB"];
  let value = bytes;
  let unit = 0;
  while (value >= 1024 && unit < units.length - 1) {
    value /= 1024;
    unit += 1;
  }
  return `${value < 10 && unit ? value.toFixed(1) : Math.round(value)} ${units[unit]}`;
}

export function formatCountdown(seconds: number): string {
  if (seconds <= 0) return "Refreshing…";
  const m = Math.floor(seconds / 60);
  const s = seconds % 60;
  return m ? `${m}:${String(s).padStart(2, "0")}` : `${s}s`;
}

export function initial(text: string | undefined, fallback = "?"): string {
  return (text || fallback).trim().slice(0, 1).toUpperCase() || fallback;
}

export type FileKind = "folder" | "image" | "video" | "audio" | "archive" | "doc" | "file";

export function fileKind(name: string, directory = false): FileKind {
  if (directory) return "folder";
  const ext = name.split(".").pop()?.toLowerCase() ?? "";
  if (["jpg", "jpeg", "png", "gif", "webp", "heic", "bmp", "svg"].includes(ext)) return "image";
  if (["mp4", "mov", "mkv", "webm", "avi", "3gp"].includes(ext)) return "video";
  if (["mp3", "m4a", "wav", "flac", "ogg", "aac", "opus"].includes(ext)) return "audio";
  if (["zip", "rar", "7z", "tar", "gz", "apk"].includes(ext)) return "archive";
  if (["pdf", "doc", "docx", "txt", "md", "xls", "xlsx", "ppt", "pptx", "csv"].includes(ext)) return "doc";
  return "file";
}

/** Loose subsequence match used by search fields and the command menu. Higher is better; -1 is no match. */
export function fuzzyScore(query: string, text: string): number {
  const q = query.trim().toLowerCase();
  if (!q) return 0;
  const t = text.toLowerCase();
  const direct = t.indexOf(q);
  if (direct >= 0) return 1000 - direct * 2 - (t.length - q.length) * 0.1;
  let score = 0;
  let at = 0;
  for (const ch of q) {
    const found = t.indexOf(ch, at);
    if (found < 0) return -1;
    score += found === at ? 6 : 1;
    at = found + 1;
  }
  return score;
}

/** Stable hue per name, so letter tiles and avatars keep their color between renders. */
export function nameHue(text: string): number {
  let hash = 0;
  for (let i = 0; i < text.length; i += 1) hash = (hash * 31 + text.charCodeAt(i)) | 0;
  return Math.abs(hash) % 360;
}
