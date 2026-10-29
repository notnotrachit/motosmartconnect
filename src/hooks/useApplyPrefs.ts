import { useEffect, useState } from "react";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { useUi } from "@/store/ui";

const darkQuery = () => window.matchMedia("(prefers-color-scheme: dark)");

/** Resolved "light" | "dark", following the system when the preference is "system". */
export function useResolvedTheme(): "light" | "dark" {
  const pref = useUi((s) => s.prefs.theme);
  const [systemDark, setSystemDark] = useState(() => darkQuery().matches);
  useEffect(() => {
    const query = darkQuery();
    const onChange = () => setSystemDark(query.matches);
    query.addEventListener("change", onChange);
    return () => query.removeEventListener("change", onChange);
  }, []);
  if (pref === "system") return systemDark ? "dark" : "light";
  return pref;
}

/** Mirror appearance preferences onto <html> data attributes and the native window. */
export function useApplyPrefs() {
  const theme = useResolvedTheme();
  const prefs = useUi((s) => s.prefs);
  const systemReduced = useReducedMotionQuery();

  useEffect(() => {
    const root = document.documentElement;
    root.dataset.theme = theme;
    root.dataset.accent = prefs.accent;
    root.dataset.density = prefs.density;
    root.dataset.motion = prefs.reduceMotion || systemReduced ? "reduced" : "full";
  }, [theme, prefs.accent, prefs.density, prefs.reduceMotion, systemReduced]);

  useEffect(() => {
    // Keeps the traffic lights and vibrancy in step with the page.
    getCurrentWindow()
      .setTheme(prefs.theme === "system" ? null : prefs.theme)
      .catch(() => {});
  }, [prefs.theme]);

  return { reducedMotion: prefs.reduceMotion || systemReduced };
}

function useReducedMotionQuery() {
  const [reduced, setReduced] = useState(() => window.matchMedia("(prefers-reduced-motion: reduce)").matches);
  useEffect(() => {
    const query = window.matchMedia("(prefers-reduced-motion: reduce)");
    const onChange = () => setReduced(query.matches);
    query.addEventListener("change", onChange);
    return () => query.removeEventListener("change", onChange);
  }, []);
  return reduced;
}
