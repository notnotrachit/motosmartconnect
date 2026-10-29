import { create } from "zustand";

export interface ConfirmOptions {
  title: string;
  body?: string;
  confirmLabel?: string;
  tone?: "default" | "danger";
}

interface ConfirmStore {
  request: (ConfirmOptions & { resolve: (ok: boolean) => void }) | null;
}

export const useConfirm = create<ConfirmStore>(() => ({ request: null }));

/** Ask the user to confirm. Resolves false when cancelled or dismissed. */
export function confirm(options: ConfirmOptions): Promise<boolean> {
  useConfirm.getState().request?.resolve(false);
  return new Promise((resolve) => useConfirm.setState({ request: { ...options, resolve } }));
}

export function settleConfirm(ok: boolean) {
  const { request } = useConfirm.getState();
  if (!request) return;
  useConfirm.setState({ request: null });
  request.resolve(ok);
}
