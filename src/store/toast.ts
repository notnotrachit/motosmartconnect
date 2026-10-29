import { create } from "zustand";

export type ToastTone = "info" | "success" | "error" | "warning";

export interface ToastAction {
  label: string;
  run: () => void;
}

export interface Toast {
  id: number;
  title: string;
  body?: string;
  tone: ToastTone;
  duration: number;
  action?: ToastAction;
  /** Increments when an identical toast is raised again; restarts its timer. */
  repeat: number;
}

interface ToastStore {
  toasts: Toast[];
  push: (toast: Omit<Toast, "id" | "repeat">) => number;
  dismiss: (id: number) => void;
}

const MAX_TOASTS = 4;
let nextId = 1;

export const useToasts = create<ToastStore>((set, get) => ({
  toasts: [],
  push: (input) => {
    const same = get().toasts.find((t) => t.title === input.title && t.body === input.body && t.tone === input.tone);
    if (same) {
      set({ toasts: get().toasts.map((t) => (t.id === same.id ? { ...t, repeat: t.repeat + 1 } : t)) });
      return same.id;
    }
    const id = nextId++;
    set({ toasts: [...get().toasts, { ...input, id, repeat: 0 }].slice(-MAX_TOASTS) });
    return id;
  },
  dismiss: (id) => set({ toasts: get().toasts.filter((t) => t.id !== id) }),
}));

type Options = Partial<Pick<Toast, "body" | "duration" | "action">>;

function make(tone: ToastTone, fallback: number) {
  return (title: string, options: Options = {}) =>
    useToasts.getState().push({ title, tone, duration: options.duration ?? fallback, body: options.body, action: options.action });
}

/** Imperative toast API usable from anywhere, including non-React code. */
export const toast = {
  info: make("info", 3500),
  success: make("success", 3200),
  warning: make("warning", 5000),
  error: make("error", 6500),
};
