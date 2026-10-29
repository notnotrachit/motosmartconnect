# Smart Connect for Mac

An unofficial Mac app for the Motorola Smart Connect app on your phone. Pair on the same Wi-Fi, then use the phone from the Mac: the screen, apps, files, notifications, and texts.

This project is not affiliated with Motorola or Lenovo.

## What you can do

The window keeps the phone on the left and the feature panels on the right.

- **Pair** by scanning the code in Smart Connect on the phone. A paired phone reconnects when Smart Connect is open. The Mac also calls it back on the local network.
- **Screen.** Start mirroring to see the phone. Click and type on the picture. Back, Home, and Recents use the phone’s keys. When the screen is idle, the well shows the phone’s wallpaper.
- **Apps.** Open an app in its own window.
- **Desktop.** Open the phone in a wide desktop view.
- **Camera.** Use the phone’s camera in this app. If OBS Virtual Camera is already installed, turning the camera on can publish that picture for other apps, and turning it off closes the OBS window this app opened.
- **Notifications.** Read them, reply, run an action, or open the app on the phone.
- **Messages.** Read recent texts and send one. The first time, allow Messages for this Mac in Smart Connect on the phone. The phone shares about the last 10 days. A picture message shows as “Photo” without the image.
- **Files.** Browse the phone and save a file, or receive a file the phone shares. Received files go to `Downloads/Smart Connect`.
- **Control.** Drive the phone from a trackpad on the Mac. The phone may ask you to allow this Mac.
- **Clipboard.** Text copies both ways after clipboard sync is turned on in Smart Connect on the phone. Anything you copy can sync while that is on, including passwords.
- **This Mac on the phone.** The phone’s device page shows this Mac’s name and battery. The lock tile locks the Mac.

Hotspot is not offered. Turning that tile on can start tethering before the Mac has a say.

## Requirements

- macOS with Xcode command line tools
- [Node.js](https://nodejs.org/) 20 or newer
- [Rust](https://rustup.rs/) stable
- The phone and the Mac on the same Wi-Fi
- Smart Connect installed on the phone

The first time macOS asks, allow incoming connections. To use the lock tile, allow this app to control System Events under Privacy & Security. Notification banners outside the window need the usual macOS notification permission. The list inside the app does not.

## Run

```bash
npm install
npm run tauri dev
```

The app listens on port 52020, or the next free port through 52040, and the pairing code uses whichever port opened. Quit any other Smart Connect host that is already listening on that port before you start this one.

A paired phone is remembered. Open Smart Connect on the phone and keep it in front when you want it to connect. There is no Lenovo account or cloud pairing.

## Keyboard

These work when the phone screen and the control pad do not have focus. Keys typed there go to the phone.

| Shortcut | Action |
| --- | --- |
| ⌘K | Command menu |
| ⌘, | Settings |
| ⌘L | Activity log |
| ⌘1–5 | Notifications, Messages, Apps, Files, Control |
| Esc | Close a sheet or menu |

Settings cover the Mac’s name, theme, accent, and the list of paired phones.

## Where things are stored

Identity, the pairing certificate, and the saved phones live in:

```text
~/Library/Application Support/MotoSmartConnect/identity.json
```

The certificate is self-signed and stays the same across launches. The phone remembers that fingerprint, so replacing the file means pairing again.

Forgetting a phone removes it from this Mac. If the phone is connected at the time, the app also tells it the pair is gone.

## Build

```bash
npm run typecheck
npm run tauri build
```

`npm run typecheck` is TypeScript only. `npm run tauri build` produces the Mac app.

Rust tests:

```bash
cd src-tauri
cargo test
```

## Project layout

The interface is React, TypeScript, and Tailwind, built by Vite. Rust and Tauri own the phone connection, pairing, and windows.

```text
src/
  components/   window layout, phone stage, panels, settings
  engine/       snapshot updates and user actions
  store/        engine state and saved preferences
  rtc/          screen and app video
  stream/       the separate window used for an app, the desktop, or the camera
src-tauri/src/  pairing, the phone connection, and each feature
```

Two pages are built: `index.html` for the main window and `stream.html` for a screen the phone opens. The video element stays put after the picture starts. Moving or hiding it makes WebKit drop the frame.
