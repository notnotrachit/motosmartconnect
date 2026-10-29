import type { MessageView } from "@/lib/types";

const DAY = 86_400_000;

function startOfDay(ms: number): number {
  const d = new Date(ms);
  d.setHours(0, 0, 0, 0);
  return d.getTime();
}

/** Compact time for the conversation list: "now", "5m", "9:41 AM", "Yesterday", "Tue", "Mar 4". */
export function listTime(ms: number, now: number): string {
  if (!ms) return "";
  const diff = now - ms;
  if (diff >= 0 && diff < 60_000) return "now";
  if (diff >= 0 && diff < 3_600_000) return `${Math.floor(diff / 60_000)}m`;
  const days = Math.round((startOfDay(now) - startOfDay(ms)) / DAY);
  if (days <= 0) return clock(ms);
  if (days === 1) return "Yesterday";
  if (days < 7) return new Date(ms).toLocaleDateString([], { weekday: "short" });
  const sameYear = new Date(ms).getFullYear() === new Date(now).getFullYear();
  return new Date(ms).toLocaleDateString([], sameYear ? { month: "short", day: "numeric" } : { year: "numeric", month: "short", day: "numeric" });
}

export function clock(ms: number): string {
  return new Date(ms).toLocaleTimeString([], { hour: "numeric", minute: "2-digit" });
}

/** Separator label inside a conversation. */
export function dayLabel(ms: number, now: number): string {
  const days = Math.round((startOfDay(now) - startOfDay(ms)) / DAY);
  if (days <= 0) return "Today";
  if (days === 1) return "Yesterday";
  if (days < 7) return new Date(ms).toLocaleDateString([], { weekday: "long" });
  return new Date(ms).toLocaleDateString([], { weekday: "short", month: "short", day: "numeric", year: "numeric" });
}

/** A title with no letters is a bare number; show a person glyph instead of a digit. */
export function isBareNumber(title: string): boolean {
  return !/\p{L}/u.test(title);
}

/** Same rules as clean_address in src-tauri/src/sms.rs, so the button only enables when the engine will accept it. */
export function validNumber(raw: string): boolean {
  const trimmed = raw.trim();
  const digits = (trimmed.match(/\d/g) ?? []).length;
  return /^[\d+\-() ]+$/.test(trimmed) && digits >= 3 && digits <= 15 && trimmed.length <= 32;
}

const GSM7 = /^[A-Za-z0-9 \r\n@£$¥èéùìòÇØøÅåΔ_ΦΓΛΩΠΨΣΘΞÆæßÉ!"#¤%&'()*+,\-./:;<=>?¡ÄÖÑÜ§¿äöñüà^{}\\[~\]|€]*$/;

/** How many SMS parts a text takes, and characters left in the current part. */
export function smsParts(text: string): { parts: number; left: number } {
  if (!text) return { parts: 0, left: 160 };
  const gsm = GSM7.test(text);
  const single = gsm ? 160 : 70;
  const multi = gsm ? 153 : 67;
  const length = [...text].length;
  if (length <= single) return { parts: 1, left: single - length };
  const parts = Math.ceil(length / multi);
  return { parts, left: parts * multi - length };
}

/** A one-time code in an incoming text, if it reads like a verification message. */
export function oneTimeCode(body: string): string {
  if (!/code|otp|verif|password|passcode|pin|login|sign.?in|confirm/i.test(body)) return "";
  return body.match(/(?<![\d-])(\d{4,8})(?![\d-])/)?.[1] ?? "";
}

export type BubblePosition = "single" | "first" | "middle" | "last";

export interface Row {
  kind: "day" | "message";
  key: string;
  label?: string;
  message?: MessageView;
  position?: BubblePosition;
  /** Show the time under this bubble. */
  stamp?: boolean;
}

const GROUP_GAP = 3 * 60_000;

/** Interleave day separators and mark each bubble's place in its run. */
export function buildRows(messages: MessageView[], now: number): Row[] {
  const rows: Row[] = [];
  let day = -1;
  messages.forEach((message, index) => {
    const prev = messages[index - 1];
    const next = messages[index + 1];
    const thisDay = message.date ? startOfDay(message.date) : day;
    if (message.date && thisDay !== day) {
      day = thisDay;
      rows.push({ kind: "day", key: `day-${thisDay}`, label: dayLabel(message.date, now) });
    }
    const joinsPrev = Boolean(prev && prev.outgoing === message.outgoing && message.date - prev.date < GROUP_GAP && startOfDay(prev.date) === thisDay);
    const joinsNext = Boolean(next && next.outgoing === message.outgoing && next.date - message.date < GROUP_GAP && startOfDay(next.date) === thisDay);
    const position: BubblePosition = joinsPrev ? (joinsNext ? "middle" : "last") : joinsNext ? "first" : "single";
    rows.push({ kind: "message", key: message.id, message, position, stamp: !joinsNext || message.pending || message.failed });
  });
  return rows;
}
