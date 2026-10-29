import { useEffect, useRef } from "react";

/**
 * Stack of open overlays (dialogs, drawers, menus). Escape closes only the
 * topmost one, so nested layers behave like native sheets.
 */
const stack: { id: symbol; close: () => void }[] = [];

function onKeyDown(event: KeyboardEvent) {
  if (event.key !== "Escape" || !stack.length) return;
  // Keys typed into the phone screen or the cross-control pad belong to the phone.
  if ((event.target as Element | null)?.closest?.("[data-forward-keys]")) return;
  event.preventDefault();
  stack[stack.length - 1].close();
}

let listening = false;

export function useLayer(open: boolean, close: () => void) {
  const closeRef = useRef(close);
  closeRef.current = close;

  useEffect(() => {
    if (!open) return;
    if (!listening) {
      document.addEventListener("keydown", onKeyDown);
      listening = true;
    }
    const entry = { id: Symbol("layer"), close: () => closeRef.current() };
    stack.push(entry);
    return () => {
      const index = stack.indexOf(entry);
      if (index >= 0) stack.splice(index, 1);
    };
  }, [open]);
}

export const hasOpenLayer = () => stack.length > 0;
