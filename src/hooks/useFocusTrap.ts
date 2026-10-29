import { useEffect, type RefObject } from "react";

const FOCUSABLE =
  'a[href], button:not([disabled]), input:not([disabled]), textarea:not([disabled]), select:not([disabled]), [tabindex]:not([tabindex="-1"])';

/**
 * Keep Tab inside `ref` while active, focus the first [data-autofocus] (or
 * focusable) element on open, and give focus back to the opener on close.
 */
export function useFocusTrap(ref: RefObject<HTMLElement | null>, active: boolean) {
  useEffect(() => {
    if (!active) return;
    const opener = document.activeElement as HTMLElement | null;
    const node = ref.current;
    const first = () => {
      if (!node) return;
      const target = node.querySelector<HTMLElement>("[data-autofocus]") ?? node.querySelector<HTMLElement>(FOCUSABLE) ?? node;
      target.focus({ preventScroll: true });
    };
    const raf = requestAnimationFrame(first);

    const onKeyDown = (event: KeyboardEvent) => {
      if (event.key !== "Tab" || !node) return;
      const items = [...node.querySelectorAll<HTMLElement>(FOCUSABLE)].filter((el) => el.offsetParent !== null);
      if (!items.length) return;
      const firstItem = items[0];
      const lastItem = items[items.length - 1];
      if (event.shiftKey && document.activeElement === firstItem) {
        event.preventDefault();
        lastItem.focus();
      } else if (!event.shiftKey && document.activeElement === lastItem) {
        event.preventDefault();
        firstItem.focus();
      }
    };
    node?.addEventListener("keydown", onKeyDown);
    return () => {
      cancelAnimationFrame(raf);
      node?.removeEventListener("keydown", onKeyDown);
      if (opener?.isConnected) opener.focus({ preventScroll: true });
    };
  }, [ref, active]);
}
