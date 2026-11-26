//! Status the phone reports about itself on service 25, `cmd 1`.
//!
//! Field names follow docs/protocol/device-status-and-tiles.md in smartconnect-linux
//! (verified on a device). Every field is optional: a message that carries only some of
//! them still updates those, and a message with none of them is ignored.

use serde::Serialize;
use serde_json::Value;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PhoneStatus {
    /// Battery percent, 0 to 100.
    pub battery: Option<i64>,
    pub charging: Option<bool>,
    pub wifi: Option<bool>,
    /// Raw value from the phone. Its scale is not documented, so the UI does not render it as bars.
    pub wifi_signal: Option<i64>,
    /// Raw value from the phone, same caveat as `wifi_signal`.
    pub data_signal: Option<i64>,
    pub bluetooth: Option<bool>,
}

/// Read a phone status message. `data` is the flat payload: `{"ver": "1", "cmd": 1, ...}`.
pub fn parse(data: &Value) -> Option<PhoneStatus> {
    let status = PhoneStatus {
        battery: data.get("battery").and_then(Value::as_i64).filter(|level| (0..=100).contains(level)),
        charging: data.get("isCharging").and_then(Value::as_bool),
        wifi: data.get("wifi").and_then(Value::as_bool),
        wifi_signal: data.get("wifiSignal").and_then(Value::as_i64),
        data_signal: data.get("dataSignal").and_then(Value::as_i64),
        bluetooth: data.get("bluetooth").and_then(Value::as_bool),
    };
    let any = status.battery.is_some()
        || status.charging.is_some()
        || status.wifi.is_some()
        || status.wifi_signal.is_some()
        || status.data_signal.is_some()
        || status.bluetooth.is_some();
    any.then_some(status)
}
