import { create } from "zustand";
import { listPhoneFolder } from "@/engine/actions";
import { errorText } from "@/lib/tauri";
import type { PhoneEntry } from "@/lib/types";

export interface BrowserState {
  opened: boolean;
  path: string;
  entries: PhoneEntry[];
  loading: boolean;
  error: string;
  /** +1 going deeper, -1 going up; drives the slide direction. */
  direction: 1 | -1;
}

const join = (parent: string, name: string) => (parent ? `${parent}/${name}` : name);
const parentOf = (path: string) => path.split("/").slice(0, -1).join("/");

// Lives outside the component so the folder survives switching tabs.
const useBrowserStore = create<BrowserState>(() => ({
  opened: false,
  path: "",
  entries: [],
  loading: false,
  error: "",
  direction: 1,
}));

let request = 0;

async function open(path: string, direction: 1 | -1 = 1) {
  const id = ++request;
  useBrowserStore.setState({ opened: true, path, loading: true, error: "", direction });
  try {
    const entries = await listPhoneFolder(path);
    if (id !== request) return;
    const sorted = [...(entries ?? [])].sort((a, b) =>
      a.directory === b.directory ? a.name.localeCompare(b.name, undefined, { sensitivity: "base" }) : a.directory ? -1 : 1,
    );
    useBrowserStore.setState({ entries: sorted, loading: false });
  } catch (error) {
    if (id !== request) return;
    useBrowserStore.setState({ entries: [], loading: false, error: errorText(error) });
  }
}

/** Folder navigation over the phone's storage, ignoring stale responses. */
export function usePhoneBrowser() {
  const state = useBrowserStore();
  return {
    state,
    open,
    up: () => open(parentOf(state.path), -1),
    reload: () => open(state.path, state.direction),
    pathOf: (name: string) => join(state.path, name),
  };
}
