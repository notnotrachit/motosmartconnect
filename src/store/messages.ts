import { create } from "zustand";
import { useUi } from "./ui";

/**
 * View state for the Messages panel that the engine does not track: the
 * compose form, per-conversation drafts, and which row is opening.
 * Kept outside components so drafts survive switching tabs.
 */
interface MessageUi {
  composing: boolean;
  to: string;
  drafts: Record<string, string>;
  opening: string;
  /** Bumped to ask the composer to take focus (e.g. after "Edit & resend"). */
  focusTick: number;
  startCompose: () => void;
  stopCompose: () => void;
  setTo: (to: string) => void;
  setDraft: (key: string, text: string) => void;
  setOpening: (key: string) => void;
  focusComposer: () => void;
}

/** Draft key used by the new-message form. */
export const NEW_DRAFT = "__new__";

export const useMessageUi = create<MessageUi>((set) => ({
  composing: false,
  to: "",
  drafts: {},
  opening: "",
  focusTick: 0,
  startCompose: () => set({ composing: true }),
  stopCompose: () => set({ composing: false }),
  setTo: (to) => set({ to }),
  setDraft: (key, text) =>
    set((s) => {
      const drafts = { ...s.drafts };
      if (text) drafts[key] = text;
      else delete drafts[key];
      return { drafts };
    }),
  setOpening: (opening) => set({ opening }),
  focusComposer: () => set((s) => ({ focusTick: s.focusTick + 1 })),
}));

/** Switch to Messages and open the new-message form. */
export function composeMessage() {
  useUi.getState().setTab("messages");
  useMessageUi.getState().startCompose();
}
