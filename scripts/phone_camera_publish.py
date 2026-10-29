#!/usr/bin/env python3
"""Serve the phone picture to OBS while the phone camera is on.

OBS starts only after POST /obs with body 1, which the webcam button sends.
POST /obs with body 0 closes the OBS this helper started. Quitting OBS is
left alone. This process never prints the picture.
"""

import io
import json
import os
import signal
import subprocess
import sys
import threading
import time
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
from pathlib import Path

HOST = "127.0.0.1"
PORT = 59731
OBS = "/Applications/OBS.app/Contents/MacOS/OBS"
SCENES = Path.home() / "Library/Application Support/obs-studio/basic/scenes"
SCENE_FILE = SCENES / "SmartConnect.json"
LOG = Path("/tmp/phone-camera-publish.log")
LOCK = Path("/tmp/phone-camera-publish.lock")
OWNED = Path("/tmp/phone-camera-obs.pid")

latest = b""
seq = 0
guard = threading.Lock()


def log(text):
    try:
        with LOG.open("a", encoding="utf-8") as handle:
            handle.write(text.rstrip() + "\n")
    except OSError:
        pass


def placeholder():
    # A plain dark frame so the camera is not empty before the phone sends a picture.
    from PIL import Image, ImageDraw

    image = Image.new("RGB", (1280, 720), (7, 20, 15))
    ImageDraw.Draw(image).text((48, 340), "Waiting for the phone camera", fill=(61, 206, 110))
    buf = io.BytesIO()
    image.save(buf, format="JPEG", quality=70)
    return buf.getvalue()


def write_scene():
    SCENES.mkdir(parents=True, exist_ok=True)
    source = "6f0e2a14-7c3a-4d1e-9b55-0a1c2d3e4f50"
    scene = "a1b2c3d4-e5f6-4789-8abc-def012345678"
    body = {
        "name": "SmartConnect",
        "sources": [
            {
                "prev_ver": 537001986,
                "name": "Backdrop",
                "uuid": "b0c0ffee-1111-4222-8333-444455556666",
                "id": "color_source",
                "versioned_id": "color_source",
                "settings": {"color": 4278255360, "width": 1920, "height": 1080},
                "mixers": 0,
                "sync": 0,
                "flags": 0,
                "volume": 1.0,
                "balance": 0.5,
                "enabled": True,
                "muted": True,
                "hotkeys": {},
                "deinterlace_mode": 0,
                "deinterlace_field_order": 0,
                "monitoring_type": 0,
                "private_settings": {},
            },
            {
                "prev_ver": 537001986,
                "name": "Phone picture",
                "uuid": source,
                "id": "ffmpeg_source",
                "versioned_id": "ffmpeg_source",
                "settings": {
                    "is_local_file": False,
                    "local_file": "",
                    "looping": False,
                    "input": f"http://{HOST}:{PORT}/cam.mjpg",
                    "input_format": "mjpeg",
                    "ffmpeg_options": "framerate=20",
                    "buffering_mb": 0,
                    "reconnect_delay_sec": 2,
                    "restart_on_activate": True,
                    "clear_on_media_end": False,
                    "close_when_inactive": False,
                    "speed_percent": 100,
                    "hw_decode": False,
                },
                "mixers": 255,
                "sync": 0,
                "flags": 0,
                "volume": 1.0,
                "balance": 0.5,
                "enabled": True,
                "muted": True,
                "push-to-mute": False,
                "push-to-mute-delay": 0,
                "push-to-talk": False,
                "push-to-talk-delay": 0,
                "hotkeys": {},
                "deinterlace_mode": 0,
                "deinterlace_field_order": 0,
                "monitoring_type": 0,
                "private_settings": {},
            },
            {
                "prev_ver": 537001986,
                "name": "Phone",
                "uuid": scene,
                "id": "scene",
                "versioned_id": "scene",
                "settings": {
                    "id_counter": 1,
                    "custom_size": False,
                    "items": [
                        {
                            "name": "Backdrop",
                            "source_uuid": "b0c0ffee-1111-4222-8333-444455556666",
                            "visible": True,
                            "locked": False,
                            "rot": 0.0,
                            "scale_ref": {"x": 1920.0, "y": 1080.0},
                            "align": 5,
                            "bounds_type": 0,
                            "bounds_align": 0,
                            "bounds_crop": False,
                            "crop_left": 0,
                            "crop_top": 0,
                            "crop_right": 0,
                            "crop_bottom": 0,
                            "id": 2,
                            "group_item_backup": False,
                            "pos": {"x": 0.0, "y": 0.0},
                            "pos_rel": {"x": 0.0, "y": 0.0},
                            "scale": {"x": 1.0, "y": 1.0},
                            "scale_rel": {"x": 1.0, "y": 1.0},
                            "bounds": {"x": 0.0, "y": 0.0},
                            "bounds_rel": {"x": 0.0, "y": 0.0},
                            "scale_filter": "disable",
                            "blend_method": "default",
                            "blend_type": "normal",
                            "show_transition": {"duration": 0},
                            "hide_transition": {"duration": 0},
                            "private_settings": {},
                        },
                        {
                            "name": "Phone picture",
                            "source_uuid": source,
                            "visible": True,
                            "locked": False,
                            "rot": 0.0,
                            "scale_ref": {"x": 1920.0, "y": 1080.0},
                            "align": 5,
                            "bounds_type": 0,
                            "bounds_align": 0,
                            "bounds_crop": False,
                            "crop_left": 0,
                            "crop_top": 0,
                            "crop_right": 0,
                            "crop_bottom": 0,
                            "id": 1,
                            "group_item_backup": False,
                            "pos": {"x": 0.0, "y": 0.0},
                            "pos_rel": {"x": 0.0, "y": 0.0},
                            "scale": {"x": 1.5, "y": 1.5},
                            "scale_rel": {"x": 1.5, "y": 1.5},
                            "bounds": {"x": 0.0, "y": 0.0},
                            "bounds_rel": {"x": 0.0, "y": 0.0},
                            "scale_filter": "disable",
                            "blend_method": "default",
                            "blend_type": "normal",
                            "show_transition": {"duration": 0},
                            "hide_transition": {"duration": 0},
                            "private_settings": {},
                        }
                    ],
                },
                "mixers": 0,
                "sync": 0,
                "flags": 0,
                "volume": 1.0,
                "balance": 0.5,
                "enabled": True,
                "muted": False,
                "hotkeys": {},
                "deinterlace_mode": 0,
                "deinterlace_field_order": 0,
                "monitoring_type": 0,
                "private_settings": {},
            },
        ],
        "current_scene": "Phone",
        "current_program_scene": "Phone",
        "canvases": [],
        "current_transition": "Cut",
        "transition_duration": 0,
        "transitions": [],
        "quick_transitions": [],
        "saved_projectors": [],
        "preview_locked": False,
        "scaling_enabled": False,
        "scaling_level": 0,
        "scaling_off_x": 0.0,
        "scaling_off_y": 0.0,
        "virtual-camera": {"type2": 3},
        "modules": {},
        "resolution": {"x": 1920, "y": 1080},
        "version": 2,
    }
    SCENE_FILE.write_text(json.dumps(body, indent=2), encoding="utf-8")


def extension_waiting():
    try:
        text = subprocess.check_output(["systemextensionsctl", "list"], text=True, stderr=subprocess.DEVNULL)
    except (OSError, subprocess.CalledProcessError):
        return False
    for line in text.splitlines():
        if "obs-studio.mac-camera-extension" in line and "waiting for user" in line:
            return True
    return False


def nudge_obs():
    """Start the virtual camera and pin the picture to the full frame."""
    try:
        import asyncio
        import base64
        import hashlib

        import websockets
    except ImportError:
        log("OBS websocket support is missing, so the virtual camera was not started.")
        return
    cfg_path = Path.home() / "Library/Application Support/obs-studio/plugin_config/obs-websocket/config.json"
    try:
        cfg = json.loads(cfg_path.read_text(encoding="utf-8"))
    except (OSError, json.JSONDecodeError):
        log("OBS websocket is not configured.")
        return
    # Starting the camera before macOS finishes activating the extension makes OBS
    # show a false "not installed" warning. The toggle in Settings can already be on.
    logs = Path.home() / "Library/Application Support/obs-studio/logs"
    for _ in range(40):
        newest = sorted(logs.glob("*.txt"), key=lambda path: path.stat().st_mtime, reverse=True)
        if newest and "activated successfully" in newest[0].read_text(encoding="utf-8", errors="replace"):
            break
        time.sleep(0.25)
    password = cfg.get("server_password") or ""
    port = int(cfg.get("server_port") or 4455)

    def auth_token(salt, challenge):
        secret = base64.b64encode(hashlib.sha256((password + salt).encode()).digest()).decode()
        return base64.b64encode(hashlib.sha256((secret + challenge).encode()).digest()).decode()

    async def run():
        for _ in range(30):
            try:
                ws = await websockets.connect(f"ws://127.0.0.1:{port}")
                break
            except OSError:
                await asyncio.sleep(0.4)
        else:
            log("OBS websocket did not open.")
            return
        async with ws:
            hello = json.loads(await ws.recv())
            ident = {"op": 1, "d": {"rpcVersion": 1}}
            info = hello.get("d", {}).get("authentication")
            if info:
                ident["d"]["authentication"] = auth_token(info["salt"], info["challenge"])
            await ws.send(json.dumps(ident))
            await ws.recv()
            request_id = 0

            async def call(kind, data=None):
                nonlocal request_id
                request_id += 1
                body = {"op": 6, "d": {"requestType": kind, "requestId": str(request_id)}}
                if data:
                    body["d"]["requestData"] = data
                await ws.send(json.dumps(body))
                while True:
                    msg = json.loads(await ws.recv())
                    if msg.get("op") == 7 and msg.get("d", {}).get("requestId") == str(request_id):
                        return msg["d"].get("requestStatus", {}), msg["d"].get("responseData") or {}

            status, data = await call("GetVirtualCamStatus")
            if not (data or {}).get("outputActive"):
                await call("StartVirtualCam")
            full = {"positionX": 0.0, "positionY": 0.0, "alignment": 5}
            for item in (1, 2):
                await call(
                    "SetSceneItemTransform",
                    {"sceneName": "Phone", "sceneItemId": item, "sceneItemTransform": full},
                )
            await call(
                "SetSceneItemTransform",
                {
                    "sceneName": "Phone",
                    "sceneItemId": 1,
                    "sceneItemTransform": {"positionX": 0.0, "positionY": 0.0, "alignment": 5, "scaleX": 1.5, "scaleY": 1.5},
                },
            )
        log("OBS Virtual Camera is sending the phone picture.")

    try:
        asyncio.run(run())
    except Exception as err:
        log(f"Could not start the virtual camera ({err.__class__.__name__}).")


obs_gate = threading.Lock()
obs_token = 0


def pid_alive(pid):
    try:
        os.kill(pid, 0)
    except OSError:
        return False
    return True


def read_owned():
    try:
        return int(OWNED.read_text(encoding="utf-8").strip())
    except (OSError, ValueError):
        return None


def obs_running():
    try:
        return subprocess.run(["pgrep", "-x", "OBS"], stdout=subprocess.DEVNULL).returncode == 0
    except OSError:
        return False


def launch_obs():
    if not Path(OBS).exists():
        log("OBS is not installed.")
        return None
    write_scene()
    if extension_waiting():
        log("OBS Virtual Camera is installed and still needs Allow in Camera Extensions.")
        subprocess.Popen(
            ["open", "x-apple.systempreferences:com.apple.LoginItems-Settings.extension"],
            stdout=subprocess.DEVNULL,
            stderr=subprocess.DEVNULL,
        )
    if obs_running():
        return None
    proc = subprocess.Popen(
        [
            OBS,
            "--collection",
            "SmartConnect",
            "--scene",
            "Phone",
            "--disable-missing-files-check",
            "--disable-updater",
        ],
        stdout=subprocess.DEVNULL,
        stderr=subprocess.DEVNULL,
        start_new_session=True,
    )
    log("Started OBS for the phone camera.")
    return proc.pid


def open_for_webcam():
    """Start OBS because the webcam button is on. Do not restart it if the user quits it."""
    global obs_token
    with obs_gate:
        obs_token += 1
        token = obs_token
        owned = read_owned()
    if owned and pid_alive(owned):
        log("OBS is already open for the phone camera.")
        nudge_obs()
        return
    started = launch_obs()
    with obs_gate:
        if token != obs_token:
            if started and pid_alive(started):
                os.kill(started, signal.SIGTERM)
            return
        if started:
            OWNED.write_text(str(started), encoding="utf-8")
        elif obs_running():
            log("OBS was already open, so closing the phone camera will leave it open.")
            OWNED.unlink(missing_ok=True)
        else:
            return
    nudge_obs()


def close_for_webcam():
    """Quit only the OBS this helper started for the phone camera."""
    global obs_token
    with obs_gate:
        obs_token += 1
        pid = read_owned()
        OWNED.unlink(missing_ok=True)
    if not pid or not pid_alive(pid):
        log("The phone camera stopped. OBS was not opened by it.")
        return
    log("Closing OBS because the phone camera stopped.")
    try:
        os.kill(pid, signal.SIGTERM)
    except OSError:
        return
    for _ in range(20):
        if not pid_alive(pid):
            return
        time.sleep(0.1)
    try:
        os.kill(pid, signal.SIGKILL)
    except OSError:
        pass


class Handler(BaseHTTPRequestHandler):
    # HTTP/1.0 so a client can read the picture stream without a length or chunked encoding.
    protocol_version = "HTTP/1.0"

    def log_message(self, fmt, *args):
        return

    def do_POST(self):
        path = self.path.split("?", 1)[0]
        if path == "/obs":
            try:
                length = int(self.headers.get("Content-Length", "0"))
            except ValueError:
                length = 0
            body = self.rfile.read(max(0, min(length, 8))).decode("utf-8", "replace").strip()
            if body == "1":
                threading.Thread(target=open_for_webcam, daemon=True).start()
            elif body == "0":
                threading.Thread(target=close_for_webcam, daemon=True).start()
            else:
                self.send_error(400)
                return
            self.send_response(204)
            self.send_header("Content-Length", "0")
            self.end_headers()
            return
        if path != "/frame":
            self.send_error(404)
            return
        try:
            length = int(self.headers.get("Content-Length", "0"))
        except ValueError:
            length = 0
        if length <= 0 or length > 2_000_000:
            self.send_error(400)
            return
        data = self.rfile.read(length)
        if len(data) >= 2 and data[0] == 0xFF and data[1] == 0xD8:
            global seq
            with guard:
                global latest
                latest = data
                seq += 1
        self.send_response(204)
        self.send_header("Content-Length", "0")
        self.end_headers()

    def do_GET(self):
        path = self.path.split("?", 1)[0]
        if path == "/health":
            body = b"ok"
            self.send_response(200)
            self.send_header("Content-Type", "text/plain")
            self.send_header("Content-Length", str(len(body)))
            self.end_headers()
            self.wfile.write(body)
            return
        if path != "/cam.mjpg":
            self.send_error(404)
            return
        self.send_response(200)
        self.send_header("Content-Type", "video/x-motion-jpeg")
        self.send_header("Cache-Control", "no-store")
        self.send_header("Connection", "close")
        self.end_headers()
        try:
            while True:
                with guard:
                    frame = latest
                if frame:
                    self.wfile.write(frame)
                    self.wfile.flush()
                time.sleep(0.05)
        except (BrokenPipeError, ConnectionResetError, OSError):
            return


def take_lock():
    import fcntl

    handle = LOCK.open("a+")
    try:
        fcntl.flock(handle.fileno(), fcntl.LOCK_EX | fcntl.LOCK_NB)
    except OSError:
        log("The phone camera publisher is already running.")
        sys.exit(0)
    return handle


def main():
    os.umask(0o077)
    held = take_lock()
    with guard:
        global latest
        latest = placeholder()
    try:
        server = ThreadingHTTPServer((HOST, PORT), Handler)
    except OSError as err:
        log(f"The camera port is busy ({err}).")
        sys.exit(0)
    threading.Thread(target=server.serve_forever, daemon=True).start()
    log(f"Listening on {HOST}:{PORT}")
    try:
        threading.Event().wait()
    finally:
        held.close()


if __name__ == "__main__":
    main()
