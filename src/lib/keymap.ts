// PC set-1 scancodes the phone expects for keyboard input.

export const SCANCODES: Record<string, number> = {
  KeyA: 0x1e, KeyB: 0x30, KeyC: 0x2e, KeyD: 0x20, KeyE: 0x12, KeyF: 0x21, KeyG: 0x22, KeyH: 0x23,
  KeyI: 0x17, KeyJ: 0x24, KeyK: 0x25, KeyL: 0x26, KeyM: 0x32, KeyN: 0x31, KeyO: 0x18, KeyP: 0x19,
  KeyQ: 0x10, KeyR: 0x13, KeyS: 0x1f, KeyT: 0x14, KeyU: 0x16, KeyV: 0x2f, KeyW: 0x11, KeyX: 0x2d,
  KeyY: 0x15, KeyZ: 0x2c, Digit1: 0x02, Digit2: 0x03, Digit3: 0x04, Digit4: 0x05, Digit5: 0x06,
  Digit6: 0x07, Digit7: 0x08, Digit8: 0x09, Digit9: 0x0a, Digit0: 0x0b, Enter: 0x1c, Escape: 0x01,
  Backspace: 0x0e, Tab: 0x0f, Space: 0x39, Minus: 0x0c, Equal: 0x0d, BracketLeft: 0x1a,
  BracketRight: 0x1b, Backslash: 0x2b, Semicolon: 0x27, Quote: 0x28, Backquote: 0x29, Comma: 0x33,
  Period: 0x34, Slash: 0x35, ShiftLeft: 0x2a, ShiftRight: 0x36, ControlLeft: 0x1d, AltLeft: 0x38,
};

/** Extended keys, sent with flag 0x100. */
export const EXTENDED: Record<string, number> = {
  ArrowUp: 0x48, ArrowDown: 0x50, ArrowLeft: 0x4b, ArrowRight: 0x4d, Home: 0x47, End: 0x4f,
  PageUp: 0x49, PageDown: 0x51, Insert: 0x52, Delete: 0x53, ControlRight: 0x1d, AltRight: 0x38,
  MetaLeft: 0x5b, MetaRight: 0x5c,
};

/** Android key codes, sent with flag 0x1000. */
export const ANDROID_KEYS = {
  back: 4,
  home: 3,
  recent: 187,
} as const;

export const ANDROID_FLAG = 0x1000;
export const EXTENDED_FLAG = 0x100;

/** Mouse button numbering the phone uses: 1 left, 2 middle, 3 right. */
export function phoneButton(button: number): number {
  if (button === 2) return 3;
  if (button === 1) return 2;
  return 1;
}
