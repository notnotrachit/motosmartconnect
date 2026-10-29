import { createRoot } from "react-dom/client";
import { StreamController } from "@/rtc/StreamController";
import { StreamApp } from "./StreamApp";
import "../index.css";

declare global {
  interface Window {
    /** Set by Rust (open_stream_window) before the page loads. */
    __STREAM_PKG__?: string;
  }
}

const controller = new StreamController(window.__STREAM_PKG__ || "");

// No StrictMode here: the picture element must be mounted exactly once.
createRoot(document.getElementById("root")!).render(<StreamApp controller={controller} />);
