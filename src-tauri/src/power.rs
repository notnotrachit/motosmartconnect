//! This Mac's battery, as the phone's device page expects it.
//!
//! Windows puts three fields on service 25 command 1: `battery` (percent),
//! `isCharging`, and `chargePlug` (1 while an adapter is attached).

use std::ffi::{CStr, c_char, c_void};
use std::ptr;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct HostPower {
    pub battery: i64,
    pub charging: bool,
    pub plugged: bool,
}

impl HostPower {
    /// A Mac with no battery is treated as plugged in and full.
    pub fn on_power() -> Self {
        Self { battery: 100, charging: false, plugged: true }
    }

    pub fn line(&self) -> String {
        let how = if self.charging {
            "charging"
        } else if self.plugged {
            "plugged in"
        } else {
            "on battery"
        };
        format!("This Mac is at {}% and {how}.", self.battery)
    }
}

pub fn read() -> HostPower {
    unsafe { read_iokit() }.unwrap_or_else(HostPower::on_power)
}

const UTF8: u32 = 0x0800_0100;
const I64: isize = 4;

#[link(name = "IOKit", kind = "framework")]
unsafe extern "C" {
    fn IOPSCopyPowerSourcesInfo() -> *const c_void;
    fn IOPSCopyPowerSourcesList(blob: *const c_void) -> *const c_void;
    fn IOPSGetPowerSourceDescription(blob: *const c_void, source: *const c_void) -> *const c_void;
}

#[link(name = "CoreFoundation", kind = "framework")]
unsafe extern "C" {
    fn CFRelease(value: *const c_void);
    fn CFArrayGetCount(array: *const c_void) -> isize;
    fn CFArrayGetValueAtIndex(array: *const c_void, index: isize) -> *const c_void;
    fn CFDictionaryGetValue(dict: *const c_void, key: *const c_void) -> *const c_void;
    fn CFStringCreateWithCString(allocator: *const c_void, text: *const c_char, encoding: u32) -> *const c_void;
    fn CFGetTypeID(value: *const c_void) -> usize;
    fn CFNumberGetTypeID() -> usize;
    fn CFBooleanGetTypeID() -> usize;
    fn CFStringGetTypeID() -> usize;
    fn CFNumberGetValue(number: *const c_void, kind: isize, into: *mut c_void) -> bool;
    fn CFBooleanGetValue(value: *const c_void) -> bool;
    fn CFStringGetCString(value: *const c_void, buffer: *mut c_char, size: isize, encoding: u32) -> bool;
}

unsafe fn key(text: &CStr) -> *const c_void {
    unsafe { CFStringCreateWithCString(ptr::null(), text.as_ptr(), UTF8) }
}

unsafe fn number(dict: *const c_void, name: &CStr) -> Option<i64> {
    let key = unsafe { key(name) };
    if key.is_null() {
        return None;
    }
    let value = unsafe { CFDictionaryGetValue(dict, key) };
    unsafe { CFRelease(key) };
    if value.is_null() || unsafe { CFGetTypeID(value) } != unsafe { CFNumberGetTypeID() } {
        return None;
    }
    let mut out = 0i64;
    if unsafe { CFNumberGetValue(value, I64, &mut out as *mut i64 as *mut c_void) } {
        Some(out)
    } else {
        None
    }
}

unsafe fn flag(dict: *const c_void, name: &CStr) -> bool {
    let key = unsafe { key(name) };
    if key.is_null() {
        return false;
    }
    let value = unsafe { CFDictionaryGetValue(dict, key) };
    unsafe { CFRelease(key) };
    !value.is_null() && unsafe { CFGetTypeID(value) } == unsafe { CFBooleanGetTypeID() } && unsafe { CFBooleanGetValue(value) }
}

unsafe fn text(dict: *const c_void, name: &CStr) -> Option<String> {
    let key = unsafe { key(name) };
    if key.is_null() {
        return None;
    }
    let value = unsafe { CFDictionaryGetValue(dict, key) };
    unsafe { CFRelease(key) };
    if value.is_null() || unsafe { CFGetTypeID(value) } != unsafe { CFStringGetTypeID() } {
        return None;
    }
    let mut buffer = [0u8; 64];
    if !unsafe { CFStringGetCString(value, buffer.as_mut_ptr().cast(), buffer.len() as isize, UTF8) } {
        return None;
    }
    CStr::from_bytes_until_nul(&buffer).ok().map(|text| text.to_string_lossy().into_owned())
}

unsafe fn read_iokit() -> Option<HostPower> {
    let blob = unsafe { IOPSCopyPowerSourcesInfo() };
    if blob.is_null() {
        return None;
    }
    let list = unsafe { IOPSCopyPowerSourcesList(blob) };
    if list.is_null() {
        unsafe { CFRelease(blob) };
        return None;
    }
    let count = unsafe { CFArrayGetCount(list) };
    let mut found = None;
    for index in 0..count {
        let source = unsafe { CFArrayGetValueAtIndex(list, index) };
        let description = unsafe { IOPSGetPowerSourceDescription(blob, source) };
        if description.is_null() {
            continue;
        }
        let kind = unsafe { text(description, c"Type") };
        if kind.as_deref() != Some("InternalBattery") {
            continue;
        }
        let current = unsafe { number(description, c"Current Capacity") };
        let max = unsafe { number(description, c"Max Capacity") }.unwrap_or(100);
        let battery = match (current, max) {
            (Some(current), max) if max > 0 => (current.saturating_mul(100) / max).clamp(0, 100),
            _ => 100,
        };
        let state = unsafe { text(description, c"Power Source State") };
        found = Some(HostPower {
            battery,
            charging: unsafe { flag(description, c"Is Charging") },
            plugged: state.as_deref() == Some("AC Power"),
        });
        break;
    }
    unsafe {
        CFRelease(list);
        CFRelease(blob);
    }
    found
}

#[cfg(test)]
mod tests {
    use super::HostPower;

    #[test]
    fn the_line_names_each_power_state() {
        assert_eq!(HostPower { battery: 80, charging: false, plugged: true }.line(), "This Mac is at 80% and plugged in.");
        assert_eq!(HostPower { battery: 64, charging: true, plugged: true }.line(), "This Mac is at 64% and charging.");
        assert_eq!(HostPower { battery: 40, charging: false, plugged: false }.line(), "This Mac is at 40% and on battery.");
    }
}
