//! Phone-facing messages for a Smart Connect PC host.
//!
//! The phone treats this machine as a Windows PC (`os` 6, `type` 4, `scType` 2).
//! Discovery frames are AES-256-GCM with the raw per-pair key. The additional
//! data is the 17-byte prefix (sender nonce id and frame type). The prefix is
//! the sender's nonce, never the receiver's.

use aes_gcm::aead::{Aead, Payload};
use aes_gcm::{Aes256Gcm, KeyInit, Nonce};
use base64::engine::general_purpose::STANDARD;
use base64::Engine;
use rand::RngCore;
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use subtle::ConstantTimeEq;

pub const APP_VER: &str = "09.0.3.013.000";
pub const VER_CODE: i64 = 903013000;
pub const QR_PREFIX: &str = "motorolardpconnection";
pub const TOKEN_TTL_SECS: u64 = 180;

pub fn random_bytes<const N: usize>() -> [u8; N] {
    let mut buf = [0u8; N];
    rand::thread_rng().fill_bytes(&mut buf);
    buf
}

pub fn b64(bytes: &[u8]) -> String {
    STANDARD.encode(bytes)
}

pub fn b64_decode(text: &str) -> Option<Vec<u8>> {
    STANDARD.decode(text.trim()).ok()
}

/// `salt(8) || SHA-256(real_id || salt)[0..8]`.
pub fn derive_nonce(real: &[u8], salt: &[u8; 8]) -> [u8; 16] {
    let mut hasher = Sha256::new();
    hasher.update(real);
    hasher.update(salt);
    let hash = hasher.finalize();
    let mut out = [0u8; 16];
    out[..8].copy_from_slice(salt);
    out[8..].copy_from_slice(&hash[..8]);
    out
}

pub fn fresh_nonce(real: &[u8]) -> [u8; 16] {
    let mut salt = [0u8; 8];
    rand::thread_rng().fill_bytes(&mut salt);
    derive_nonce(real, &salt)
}

pub fn nonce_matches(nonce: &[u8], real: &[u8]) -> bool {
    if nonce.len() != 16 || real.is_empty() {
        return false;
    }
    let mut salt = [0u8; 8];
    salt.copy_from_slice(&nonce[..8]);
    let derived = derive_nonce(real, &salt);
    bool::from(derived.as_slice().ct_eq(nonce))
}

pub fn mint_token() -> String {
    let raw = random_bytes::<32>();
    let encoded = STANDARD.encode(raw);
    encoded
        .chars()
        .map(|ch| {
            if ch == '=' || ch == '+' || ch == '/' {
                random_alnum()
            } else {
                ch
            }
        })
        .collect()
}

fn random_alnum() -> char {
    const SET: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789";
    let mut byte = [0u8; 1];
    rand::thread_rng().fill_bytes(&mut byte);
    SET[(byte[0] as usize) % SET.len()] as char
}

pub fn tokens_equal(left: &str, right: &str) -> bool {
    if left.len() != right.len() {
        return false;
    }
    bool::from(left.as_bytes().ct_eq(right.as_bytes()))
}

pub fn mint_pair_key() -> String {
    b64(&random_bytes::<32>())
}

pub fn fingerprint(der: &[u8]) -> (String, String) {
    let digest = Sha256::digest(der);
    (b64(&digest), hex::encode(digest))
}

pub fn qr_payload(nonce_b64: &str, real_b64: &str, name: &str, token: &str, fp_b64: &str, port: u16, ips: &[String]) -> String {
    let wifi: Vec<&str> = ips.iter().map(String::as_str).collect();
    let device = json!({
        "deviceId": nonce_b64,
        "realDeviceId": real_b64,
        "deviceName": name,
        "os": 6,
        "type": 4,
        "verCode": VER_CODE,
        "verName": APP_VER,
        "bleAddress": ""
    });
    let body = json!({
        "wsData": {
            "cmd": 1,
            "ver": 2,
            "appVer": APP_VER,
            "i3": 1,
            "deviceId": nonce_b64,
            "realDeviceId": real_b64,
            "deviceName": name,
            "serviceId": 19,
            "token": token,
            "fp": fp_b64,
            "wsPort": port,
            "policies": [{ "wifi": wifi }],
            "os": 6,
            "type": 4,
            "verCode": VER_CODE,
            "verName": APP_VER,
            "bleAddress": "",
            "device": device
        },
        "version": "3.0.0"
    });
    format!("{QR_PREFIX}{body}")
}

pub fn qr_svg(payload: &str) -> Result<String, String> {
    let code = qrcode::QrCode::new(payload.as_bytes()).map_err(|err| err.to_string())?;
    Ok(code.render::<qrcode::render::svg::Color>().min_dimensions(280, 280).build())
}

fn service_versions() -> Value {
    json!({
        "0": 3, "1": 10, "3": 3, "9": 4, "13": 1,
        "15": 16, "22": 2, "25": 1, "26": 1, "202": 1
    })
}

fn caps() -> Value {
    json!({ "1": 2, "3": 1, "9": 3, "15": 3, "16": 3, "26": 2, "202": 1 })
}

fn min_ver_code() -> Value {
    json!({ "1": 7000000, "2": 700000000, "3": 7000000, "4": 7000000 })
}

fn envelope(nonce_b64: &str, service: i64, msg_id: u64, data: Value) -> String {
    json!({
        "deviceId": nonce_b64,
        "srcServiceId": service,
        "serviceId": service,
        "msgId": msg_id,
        "data": data
    })
    .to_string()
}

pub fn verify_message(nonce_b64: &str, real_b64: &str, name: &str, sn: &str, key: &str, msg_id: u64) -> String {
    let device = json!({
        "deviceId": nonce_b64,
        "realDeviceId": real_b64,
        "deviceName": name,
        "os": 6,
        "type": 4,
        "scType": 2,
        "verCode": VER_CODE,
        "verName": APP_VER,
        "appVer": APP_VER,
        "minVerCode": min_ver_code(),
        "sn": sn,
        "Sku": "Mac",
        "sku": "Mac",
        "cap": 0,
        "caps": caps(),
        "serviceVersions": service_versions(),
        "key": key,
        "isPaired": true
    });
    envelope(
        nonce_b64,
        0,
        msg_id,
        json!({
            "ver": 3,
            "cmd": 0,
            "token": "OK",
            "paired": 1,
            "connType": 1,
            "device": device,
            "serviceVersions": service_versions()
        }),
    )
}

/// `device.deviceId` is the stable id. The phone keys the pair on that field.
pub fn exchange_message(nonce_b64: &str, real_b64: &str, name: &str, sn: &str, key: &str, msg_id: u64) -> String {
    envelope(
        nonce_b64,
        0,
        msg_id,
        json!({
            "ver": 3,
            "cmd": 1,
            "paired": 1,
            "device": {
                "deviceId": real_b64,
                "realDeviceId": real_b64,
                "deviceName": name,
                "key": key,
                "sn": sn,
                "cap": 0,
                "os": 6,
                "type": 4,
                "i3": 1,
                "verCode": VER_CODE,
                "verName": APP_VER
            }
        }),
    )
}

/// Local forget. The phone's handling of cmd 2 beyond the command number is inferred.
pub fn delete_message(nonce_b64: &str, real_b64: &str, msg_id: u64) -> String {
    envelope(
        nonce_b64,
        0,
        msg_id,
        json!({
            "ver": 3,
            "cmd": 2,
            "paired": 0,
            "device": { "deviceId": real_b64, "realDeviceId": real_b64 }
        }),
    )
}

/// Tile map. Hotspot (13) stays off so the phone does not start tethering.
/// Webcam (3) is on. The phone shows that tile and the picture opens in this app.
/// Bit 8 on service 26 means a cross-control session is active. The phone replaces the whole map,
/// so 15 and 22 stay in the same message. Bit 16 is never set.
pub fn status_message(nonce_b64: &str, name: &str, msg_id: u64) -> String {
    status_message_for(nonce_b64, name, msg_id, false, crate::power::HostPower { battery: 100, charging: false, plugged: false })
}

/// Ask the phone to share its wallpaper. Windows sends this when `wallpaperState` is not 7.
/// The phone then asks the person to allow it, and pushes the picture afterwards.
pub fn wallpaper_request(nonce_b64: &str, msg_id: u64) -> String {
    envelope(nonce_b64, 25, msg_id, json!({ "ver": "1", "cmd": 4 }))
}

pub fn status_message_for(nonce_b64: &str, name: &str, msg_id: u64, cross_active: bool, power: crate::power::HostPower) -> String {
    let cross = if cross_active { 1 | 8 } else { 1 };
    envelope(
        nonce_b64,
        25,
        msg_id,
        json!({
            "ver": "1",
            "cmd": 1,
            "cap": { "3": 1, "15": 1, "22": 1, "26": cross },
            "battery": power.battery,
            "isCharging": power.charging,
            "chargePlug": if power.plugged { 1 } else { 0 },
            "deviceName": name,
            "realDeviceName": name,
            "model": "Mac"
        }),
    )
}

pub fn control_message(nonce_b64: &str, service: i64, msg_id: u64, data: &Value) -> String {
    unquote_session_ids(&envelope(nonce_b64, service, msg_id, data.clone()))
}

/// Keep integers with 16 or more digits as strings so a 64-bit session id survives parsing.
pub fn quote_wide_integers(input: &str) -> String {
    let bytes = input.as_bytes();
    let mut out = Vec::with_capacity(input.len() + 8);
    let mut index = 0;
    let mut in_string = false;
    let mut escape = false;
    while index < bytes.len() {
        let byte = bytes[index];
        if in_string {
            out.push(byte);
            if escape {
                escape = false;
            } else if byte == b'\\' {
                escape = true;
            } else if byte == b'"' {
                in_string = false;
            }
            index += 1;
            continue;
        }
        if byte == b'"' {
            in_string = true;
            out.push(byte);
            index += 1;
            continue;
        }
        if byte == b'-' || byte.is_ascii_digit() {
            let start = index;
            if byte == b'-' {
                index += 1;
            }
            let digits_at = index;
            while index < bytes.len() && bytes[index].is_ascii_digit() {
                index += 1;
            }
            let digits = index - digits_at;
            let prev_ok = start == 0 || matches!(bytes[start - 1], b':' | b'[' | b',' | b' ' | b'\n' | b'\t');
            let next_ok = index == bytes.len() || matches!(bytes[index], b',' | b'}' | b']' | b' ' | b'\n' | b'\r' | b'\t');
            if prev_ok && next_ok && digits >= 16 {
                out.push(b'"');
                out.extend_from_slice(&bytes[start..index]);
                out.push(b'"');
            } else {
                out.extend_from_slice(&bytes[start..index]);
            }
            continue;
        }
        out.push(byte);
        index += 1;
    }
    String::from_utf8(out).unwrap_or_else(|_| input.to_string())
}

/// `sessionId` is a 64-bit integer on the wire. The web view and this process pass it as a string.
pub fn unquote_session_ids(input: &str) -> String {
    let bytes = input.as_bytes();
    let needle = b"\"sessionId\":\"";
    let mut out = Vec::with_capacity(input.len());
    let mut index = 0;
    while index < bytes.len() {
        if bytes[index..].starts_with(needle) {
            let digits_at = index + needle.len();
            let mut end = digits_at;
            if end < bytes.len() && bytes[end] == b'-' {
                end += 1;
            }
            let number_at = end;
            while end < bytes.len() && bytes[end].is_ascii_digit() {
                end += 1;
            }
            if end > number_at && end < bytes.len() && bytes[end] == b'"' {
                out.extend_from_slice(&bytes[index..index + "\"sessionId\":".len()]);
                out.extend_from_slice(&bytes[digits_at..end]);
                index = end + 1;
                continue;
            }
        }
        out.push(bytes[index]);
        index += 1;
    }
    String::from_utf8(out).unwrap_or_else(|_| input.to_string())
}

pub fn invite_json(nonce_b64: &str, name: &str, fp_b64: &str, token: &str, port: u16, ips: &[String]) -> String {
    let wifi: Vec<&str> = ips.iter().map(String::as_str).collect();
    json!({
        "ver": 2,
        "cmd": 1,
        "deviceId": nonce_b64,
        "deviceName": name,
        "fp": fp_b64,
        "token": token,
        "policies": [{ "wifi": wifi }],
        "wsPort": port,
        "appVer": APP_VER,
        "verCode": VER_CODE,
        "os": 6,
        "scType": 2,
        "type": 4
    })
    .to_string()
}

pub fn data_object(message: &Value) -> Option<Value> {
    match &message["data"] {
        Value::Object(_) => Some(message["data"].clone()),
        Value::String(text) => serde_json::from_str(&quote_wide_integers(text)).ok(),
        _ => None,
    }
}

pub fn json_cmd(value: &Value) -> Option<i64> {
    value.get("cmd").and_then(|item| item.as_i64())
}

/// Seal a discovery frame. Layout: nonce id (16) | type 1 | iv (12) | ciphertext | tag (16).
pub fn seal_frame(key_b64: &str, sender_nonce: &[u8; 16], plaintext: &[u8]) -> Result<Vec<u8>, String> {
    seal_frame_with_iv(key_b64, sender_nonce, &random_bytes::<12>(), plaintext)
}

pub fn seal_frame_with_iv(key_b64: &str, sender_nonce: &[u8; 16], iv: &[u8; 12], plaintext: &[u8]) -> Result<Vec<u8>, String> {
    let key = b64_decode(key_b64).ok_or("pair key is not base64")?;
    if key.len() != 32 {
        return Err("pair key must be 32 bytes".into());
    }
    let cipher = Aes256Gcm::new_from_slice(&key).map_err(|err| err.to_string())?;
    let mut frame = Vec::with_capacity(16 + 1 + 12 + plaintext.len() + 16);
    frame.extend_from_slice(sender_nonce);
    frame.push(1);
    frame.extend_from_slice(iv);
    let encrypted = cipher
        .encrypt(
            Nonce::from_slice(iv),
            Payload { msg: plaintext, aad: &frame[..17] },
        )
        .map_err(|err| err.to_string())?;
    frame.extend_from_slice(&encrypted);
    Ok(frame)
}

#[derive(Debug)]
pub enum OpenedFrame {
    Payload(Vec<u8>),
    CannotDecrypt,
    Ignored,
}

pub fn open_frame(key_b64: &str, frame: &[u8]) -> Result<OpenedFrame, String> {
    if frame.len() < 16 + 1 + 12 + 16 {
        return Err("discovery frame is short".into());
    }
    match frame[16] {
        1 => {}
        3 => return Ok(OpenedFrame::CannotDecrypt),
        _ => return Ok(OpenedFrame::Ignored),
    }
    let key = b64_decode(key_b64).ok_or("pair key is not base64")?;
    if key.len() != 32 {
        return Err("pair key must be 32 bytes".into());
    }
    let cipher = Aes256Gcm::new_from_slice(&key).map_err(|err| err.to_string())?;
    let iv = &frame[17..29];
    let body = &frame[29..];
    match cipher.decrypt(Nonce::from_slice(iv), Payload { msg: body, aad: &frame[..17] }) {
        Ok(plain) => Ok(OpenedFrame::Payload(plain)),
        Err(_) => Ok(OpenedFrame::CannotDecrypt),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn nonce_vector() {
        let real: Vec<u8> = (0..16).collect();
        assert_eq!(b64(&real), "AAECAwQFBgcICQoLDA0ODw==");
        let salt = [0x10, 0x11, 0x12, 0x13, 0x14, 0x15, 0x16, 0x17];
        let nonce = derive_nonce(&real, &salt);
        assert_eq!(hex::encode(nonce), "10111213141516171d64add2a6388367");
        assert_eq!(b64(&nonce), "EBESExQVFhcdZK3SpjiDZw==");
        assert!(nonce_matches(&nonce, &real));
        assert!(!nonce_matches(&nonce, &[1u8; 16]));
    }

    #[test]
    fn discovery_frame_vector() {
        let key = "AAECAwQFBgcICQoLDA0ODxAREhMUFRYXGBkaGxwdHh8=";
        let nonce: [u8; 16] = core::array::from_fn(|index| 0xa0 + index as u8);
        let iv: [u8; 12] = core::array::from_fn(|index| index as u8);
        let frame = seal_frame_with_iv(key, &nonce, &iv, br#"{"cmd":1}"#).unwrap();
        let expected = "a0a1a2a3a4a5a6a7a8a9aaabacadaeaf01000102030405060708090a0b3c20b576a1c7f82af0f7245b9e966e997f676a43dd38cacf53";
        assert_eq!(hex::encode(&frame), expected);
        match open_frame(key, &frame).unwrap() {
            OpenedFrame::Payload(plain) => assert_eq!(plain, br#"{"cmd":1}"#),
            other => panic!("unexpected {other:?}"),
        }
    }

    #[test]
    fn token_alphabet_and_qr_shape() {
        let token = mint_token();
        assert!(!token.contains(['=', '+', '/']));
        assert!(token.len() >= 42);
        let qr = qr_payload("nonce", "real", "Desk", &token, "fp==", 52020, &["192.168.1.20".into()]);
        assert!(qr.starts_with("motorolardpconnection{"));
        let json: Value = serde_json::from_str(qr.trim_start_matches(QR_PREFIX)).unwrap();
        assert_eq!(json["version"], "3.0.0");
        assert_eq!(json["wsData"]["cmd"], 1);
        assert_eq!(json["wsData"]["os"], 6);
        assert_eq!(json["wsData"]["type"], 4);
        assert_eq!(json["wsData"]["serviceId"], 19);
        assert_eq!(json["wsData"]["policies"][0]["wifi"][0], "192.168.1.20");
        assert_eq!(json["wsData"]["device"]["verName"], APP_VER);
    }

    #[test]
    fn exchange_uses_stable_id() {
        let text = exchange_message("nonce", "real-id", "Desk", "sn-1", "key", 3);
        let json: Value = serde_json::from_str(&text).unwrap();
        assert_eq!(json["serviceId"], 0);
        assert_eq!(json["data"]["cmd"], 1);
        assert_eq!(json["data"]["device"]["deviceId"], "real-id");
        assert_eq!(json["data"]["device"]["verName"], APP_VER);
        let verify: Value = serde_json::from_str(&verify_message("nonce", "real-id", "Desk", "sn-1", "key", 1)).unwrap();
        assert_eq!(verify["data"]["token"], "OK");
        assert_eq!(verify["data"]["device"]["deviceId"], "nonce");
        assert_eq!(verify["data"]["device"]["realDeviceId"], "real-id");
        assert!(verify["data"]["device"]["verName"].is_string());
        let status: Value = serde_json::from_str(&status_message("nonce", "Desk", 2)).unwrap();
        assert_eq!(status["serviceId"], 25);
        assert!(status["data"]["cap"].get("13").is_none());
        assert_eq!(status["data"]["cap"]["22"], 1);
        assert_eq!(status["data"]["cap"]["26"], 1);
        let live: Value = serde_json::from_str(&status_message_for(
            "nonce",
            "Desk",
            2,
            true,
            crate::power::HostPower { battery: 80, charging: false, plugged: true },
        ))
        .unwrap();
        assert_eq!(live["data"]["battery"], 80);
        assert_eq!(live["data"]["isCharging"], false);
        assert_eq!(live["data"]["chargePlug"], 1);
        assert_eq!(live["data"]["cap"]["26"], 9);
        let ask: Value = serde_json::from_str(&wallpaper_request("nonce", 9)).unwrap();
        assert_eq!(ask["serviceId"], 25);
        assert_eq!(ask["data"]["cmd"], 4);
        assert_eq!(live["data"]["cap"]["15"], 1);
        assert_eq!(live["data"]["cap"]["22"], 1);
        assert!(live["data"]["cap"].get("13").is_none());
    }

    #[test]
    fn wide_session_ids_survive_a_round_trip() {
        let raw = r#"{"cmd":3,"sessionId":1700000000000000001,"nested":{"n":12},"webRTCData":"{\"sessionId\":\"999\"}"}"#;
        let quoted = quote_wide_integers(raw);
        let value: Value = serde_json::from_str(&quoted).unwrap();
        assert_eq!(value["sessionId"], "1700000000000000001");
        assert_eq!(value["nested"]["n"], 12);
        let sent = control_message("nonce", 15, 4, &json!({
            "cmd": 4,
            "sessionId": "1700000000000000001",
            "webRTCData": "{\"type\":\"answer\",\"sdp\":\"v=0\"}"
        }));
        assert!(sent.contains("\"sessionId\":1700000000000000001"));
        assert!(sent.contains("\"type\":\"answer\"") || sent.contains("answer"));
        assert!(!sent.contains("\"sessionId\":\"1700000000000000001\""));
    }
}
