//! Dev builds are a raw binary. macOS will not post a notification, or ask for
//! Bluetooth, for a process that is not a signed app bundle. Before the window
//! opens, a debug launch copies itself into a small bundle and ad-hoc signs
//! that bundle so TCC can read Info.plist.

use std::os::unix::process::CommandExt;
use std::path::{Path, PathBuf};

const MARK: &str = "SC_BUNDLE_ADOPTED";

pub fn is_bundled() -> bool {
    std::env::current_exe()
        .ok()
        .map(|path| path.to_string_lossy().contains(".app/Contents/MacOS/"))
        .unwrap_or(false)
}

/// Replace this process with the same binary launched from a `.app` bundle.
/// Does nothing when this process is already that bundle.
pub fn adopt_bundle() {
    if std::env::var_os(MARK).is_some() || is_bundled() {
        return;
    }
    let Ok(exe) = std::env::current_exe() else {
        return;
    };
    let Some(bundle_exe) = prepare(&exe) else {
        return;
    };
    let mut cmd = std::process::Command::new(&bundle_exe);
    cmd.args(std::env::args().skip(1)).env(MARK, "1");
    // `exec` keeps this process id, so the dev watcher still tracks the app.
    let _ = cmd.exec();
}

fn prepare(exe: &Path) -> Option<PathBuf> {
    let name = exe.file_name()?.to_str()?;
    let root = exe.parent()?.join("Smart Connect.app");
    let macos = root.join("Contents/MacOS");
    let resources = root.join("Contents/Resources");
    std::fs::create_dir_all(&macos).ok()?;
    std::fs::create_dir_all(&resources).ok()?;
    std::fs::write(root.join("Contents/Info.plist"), plist(name)).ok()?;
    let icon = Path::new(env!("CARGO_MANIFEST_DIR")).join("icons/icon.icns");
    if icon.is_file() {
        let _ = std::fs::copy(&icon, resources.join("AppIcon.icns"));
    }
    let dest = macos.join(name);
    // A hard link is the cargo binary. codesign cannot rewrite a mapped file,
    // and a linker signature leaves Info.plist unbound, so Bluetooth never prompts.
    let _ = std::fs::remove_file(&dest);
    std::fs::copy(exe, &dest).ok()?;
    sign_bundle(&root);
    Some(dest)
}

/// Bind Info.plist into an ad-hoc signature. The identifier comes from the plist.
fn sign_bundle(root: &Path) {
    let result = std::process::Command::new("/usr/bin/codesign")
        .args(["--force", "--sign", "-", "--timestamp=none"])
        .arg(root)
        .output();
    match result {
        Ok(output) if output.status.success() => {}
        Ok(output) => {
            let err = String::from_utf8_lossy(&output.stderr);
            eprintln!("Smart Connect: debug bundle was not signed ({err}). macOS will not ask for Bluetooth.");
        }
        Err(err) => {
            eprintln!("Smart Connect: debug bundle was not signed ({err}). macOS will not ask for Bluetooth.");
        }
    }
}

fn plist(executable: &str) -> String {
    format!(
        r#"<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
  <key>CFBundleIdentifier</key>
  <string>com.rachit.motosmartconnect</string>
  <key>CFBundleName</key>
  <string>Smart Connect</string>
  <key>CFBundleDisplayName</key>
  <string>Smart Connect</string>
  <key>CFBundleExecutable</key>
  <string>{executable}</string>
  <key>CFBundleIconFile</key>
  <string>AppIcon</string>
  <key>CFBundlePackageType</key>
  <string>APPL</string>
  <key>CFBundleShortVersionString</key>
  <string>0.1.0</string>
  <key>CFBundleVersion</key>
  <string>0.1.0</string>
  <key>LSMinimumSystemVersion</key>
  <string>11.0</string>
  <key>NSHighResolutionCapable</key>
  <true/>
  <key>NSBluetoothAlwaysUsageDescription</key>
  <string>Smart Connect uses Bluetooth to wake your paired phone. It does not turn Bluetooth on.</string>
  <key>NSLocalNetworkUsageDescription</key>
  <string>Smart Connect finds your phone on the local network and accepts its connection.</string>
  <key>NSBonjourServices</key>
  <array>
    <string>_readyformdns._tcp</string>
  </array>
</dict>
</plist>
"#
    )
}
