//! Internet tunnel. After account sign-in, signaling and TURN come from the
//! cloud broker. The phone's session bytes are handed to the local listener,
//! so screen, messages, and files keep the same Ready For session.

use crate::account::{channels_from_policy, nrt_websocket_url, policy_url, service_base};
use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use base64::Engine;
use bytes::Bytes;
use futures_util::StreamExt;
use serde::Deserialize;
use serde_json::{json, Value};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;
use tokio::sync::{mpsc, Mutex};
use tokio_tungstenite::{connect_async, tungstenite::client::IntoClientRequest, tungstenite::Message};
use webrtc::api::setting_engine::SettingEngine;
use webrtc::api::APIBuilder;
use webrtc::data_channel::data_channel_message::DataChannelMessage;
use webrtc::data_channel::RTCDataChannel;
use webrtc::ice_transport::ice_candidate::RTCIceCandidateInit;
use webrtc::ice_transport::ice_credential_type::RTCIceCredentialType;
use webrtc::ice_transport::ice_server::RTCIceServer;
use webrtc::peer_connection::configuration::RTCConfiguration;
use webrtc::peer_connection::sdp::session_description::RTCSessionDescription;
use webrtc::peer_connection::RTCPeerConnection;
use webrtc_ice::mdns::MulticastDnsMode;
use webrtc_ice::network_type::NetworkType;

static TUNNEL: Mutex<Option<Arc<RTCPeerConnection>>> = Mutex::const_new(None);

pub fn stop() {
    let previous = TUNNEL.try_lock().ok().and_then(|mut guard| guard.take());
    if let Some(peer) = previous {
        tauri::async_runtime::spawn(async move {
            let _ = peer.close().await;
        });
    }
}

pub async fn open_tunnel(
    token: &str,
    base: &str,
    _signaling: &str,
    our_name: &str,
    our_id: &str,
    target_id: &str,
    port: u16,
) -> Result<String, String> {
    let mesh_id = mesh_device_id(our_id);
    register_device(token, base, our_name, our_id, &mesh_id).await?;
    let ice = turn_servers(token, base).await.unwrap_or_default();
    if ice.is_empty() {
        return Err("The cloud broker did not provide a relay. The phone may be offline, or this session was not accepted.".to_string());
    }
    let channels = nrt_channels(base)
        .await
        .map_err(|_| "Could not reach the cloud signaling service.".to_string())?;
    if channels.is_empty() {
        return Err("The cloud signaling service did not provide a channel.".to_string());
    }
    let url = nrt_websocket_url(base, &channels.join(","));
    let request = url
        .into_client_request()
        .map_err(|_| "The signaling address is not valid.".to_string())?;
    let mut request = request;
    request
        .headers_mut()
        .insert(
            "Authorization",
            format!("Bearer {token}").parse().map_err(|_| "Could not authorize the tunnel.".to_string())?,
        );
    request
        .headers_mut()
        .insert("User-Agent", MESH_USER_AGENT.parse().map_err(|_| "Could not authorize the tunnel.".to_string())?);
    let (socket, _) = connect_async(request)
        .await
        .map_err(|_| "Could not reach the cloud signaling service.".to_string())?;
    let (_write, mut read) = socket.split();
    let (outbound, mut outbound_rx) = mpsc::unbounded_channel::<Outbound>();
    let post_token = token.to_string();
    let post_base = base.to_string();
    let post_target = target_id.to_string();
    let post_source = mesh_id.clone();
    let writer = tokio::spawn(async move {
        while let Some(item) = outbound_rx.recv().await {
            let _ = send_signal(&post_token, &post_base, &post_target, &post_source, &item).await;
        }
    });

    let mut setting = SettingEngine::default();
    setting.set_network_types(vec![NetworkType::Udp4]);
    setting.set_ice_multicast_dns_mode(MulticastDnsMode::Disabled);
    let peer = APIBuilder::new()
        .with_setting_engine(setting)
        .build()
        .new_peer_connection(RTCConfiguration { ice_servers: ice, ..Default::default() })
        .await
        .map_err(|err| {
            eprintln!("cloud: peer: {err}");
            "Could not start the tunnel.".to_string()
        })?;
    let peer = Arc::new(peer);
    {
        let mut guard = TUNNEL.lock().await;
        if let Some(previous) = guard.take() {
            let _ = previous.close().await;
        }
        *guard = Some(peer.clone());
    }

    let opened = Arc::new(tokio::sync::Notify::new());
    let opened_wait = opened.clone();
    let local_port = port;
    let bridged = Arc::new(AtomicBool::new(false));
    peer.on_data_channel(Box::new(move |channel| {
        let opened = opened.clone();
        let bridged = bridged.clone();
        Box::pin(async move {
            if bridged.swap(true, Ordering::SeqCst) {
                return;
            }
            eprintln!("cloud: data channel");
            bridge(channel, local_port, opened);
        })
    }));
    peer.on_ice_connection_state_change(Box::new(|state| {
        eprintln!("cloud: ice {state}");
        Box::pin(async {})
    }));
    let signal = outbound.clone();
    peer.on_ice_candidate(Box::new(move |candidate| {
        let signal = signal.clone();
        Box::pin(async move {
            let Some(candidate) = candidate else {
                return;
            };
            let Ok(init) = candidate.to_json() else {
                return;
            };
            let _ = signal.send(Outbound::Candidate {
                candidate: init.candidate,
                sdp_mid: init.sdp_mid,
                sdp_mline_index: init.sdp_mline_index,
            });
        })
    }));

    let pending_ice = Arc::new(tokio::sync::Mutex::new(Vec::<RTCIceCandidateInit>::new()));
    let peer_read = peer.clone();
    let opened_read = opened_wait.clone();
    let target = target_id.to_string();
    let incoming = outbound.clone();
    let pending_read = pending_ice.clone();
    tokio::spawn(async move {
        while let Some(message) = read.next().await {
            let Ok(message) = message else {
                break;
            };
            let text = match message {
                Message::Text(text) => text.to_string(),
                Message::Binary(bytes) => String::from_utf8(bytes.to_vec()).unwrap_or_default(),
                _ => continue,
            };
            let normalized = normalize_signal(&text);
            note_notification(&text, normalized.len());
            for item in normalized {
                handle_signal(
                    &peer_read,
                    &incoming,
                    &target,
                    &item,
                    port,
                    opened_read.clone(),
                    &pending_read,
                )
                .await;
            }
        }
    });

    // An idle phone starts its own tunnel only after an offer whose ice-ufrag is empty.
    send_signal(token, base, target_id, &mesh_id, &Outbound::Offer(WAKE_OFFER.to_string()))
        .await
        .map_err(|_| "The cloud broker did not accept the tunnel offer.".to_string())?;
    eprintln!("cloud: wake offer sent");

    // The phone's offer is also queued for this device. The socket above only
    // shows a copy of what we sent until that queue is polled.
    let poll_token = token.to_string();
    let poll_base = base.to_string();
    let poll_mesh = mesh_id.clone();
    let poll_peer = peer.clone();
    let poll_out = outbound.clone();
    let poll_target = target_id.to_string();
    let poll_opened = opened_wait.clone();
    let poll_ice = pending_ice.clone();
    let poller = tokio::spawn(async move {
        loop {
            poll_inbox(
                &poll_token,
                &poll_base,
                &poll_mesh,
                &poll_peer,
                &poll_out,
                &poll_target,
                port,
                poll_opened.clone(),
                &poll_ice,
            )
            .await;
            tokio::time::sleep(Duration::from_secs(2)).await;
        }
    });

    let connected = tokio::time::timeout(Duration::from_secs(40), opened_wait.notified()).await.is_ok();
    poller.abort();
    if !connected {
        writer.abort();
        stop();
        return Err(
            "The broker accepted the wake-up, but the phone did not send its own tunnel offer. It still has to be confirmed with the phone off this Wi-Fi."
                .to_string(),
        );
    }
    Ok("The internet tunnel is open on this Mac. Confirm it with the phone off this Wi-Fi.".to_string())
}

/// Copy bytes only after the data channel is open. A local TCP connect before
/// that would look like a phone to the listener, and would report success early.
fn bridge(channel: Arc<RTCDataChannel>, port: u16, opened: Arc<tokio::sync::Notify>) {
    let opened_channel = channel.clone();
    channel.on_open(Box::new(move || {
        let channel = opened_channel;
        Box::pin(async move {
            let (tx, mut rx) = mpsc::unbounded_channel::<Bytes>();
            channel.on_message(Box::new(move |message: DataChannelMessage| {
                let _ = tx.send(message.data);
                Box::pin(async {})
            }));
            let channel_out = channel.clone();
            let Ok(stream) = TcpStream::connect(("127.0.0.1", port)).await else {
                eprintln!("cloud: the local listener is not available");
                return;
            };
            opened.notify_one();
            let (mut read, mut write) = stream.into_split();
            let writer = tokio::spawn(async move {
                while let Some(buf) = rx.recv().await {
                    if write.write_all(&buf).await.is_err() {
                        break;
                    }
                }
            });
            let mut buf = vec![0u8; 16 * 1024];
            loop {
                match read.read(&mut buf).await {
                    Ok(0) | Err(_) => break,
                    Ok(n) => {
                        if channel_out.send(&Bytes::copy_from_slice(&buf[..n])).await.is_err() {
                            break;
                        }
                    }
                }
            }
            writer.abort();
        })
    }));
}

async fn handle_signal(
    peer: &RTCPeerConnection,
    outbound: &mpsc::UnboundedSender<Outbound>,
    target: &str,
    text: &str,
    port: u16,
    opened: Arc<tokio::sync::Notify>,
    pending_ice: &tokio::sync::Mutex<Vec<RTCIceCandidateInit>>,
) {
    let Ok(value) = serde_json::from_str::<Signal>(text) else {
        return;
    };
    let kind = value.kind.or(value.event_type).unwrap_or_default();
    if value.device_id.as_deref().is_some_and(|id| id != target && !id.is_empty()) && kind != "offer" {
        // Keep offers even when the id field differs; the broker may use its own id.
    }
    match kind.as_str() {
        "offer" => {
            let Some(sdp) = value.sdp else {
                return;
            };
            if !usable_offer(&sdp) {
                eprintln!(
                    "cloud: ignored offer bytes={} media={} ufrag={}",
                    sdp.len(),
                    sdp_has_media(&sdp),
                    ufrag_class(&sdp)
                );
                return;
            }
            let Ok(remote) = RTCSessionDescription::offer(sdp) else {
                return;
            };
            if peer.set_remote_description(remote).await.is_err() {
                eprintln!("cloud: offer was not accepted");
                return;
            }
            flush_ice(peer, pending_ice).await;
            let Ok(answer) = peer.create_answer(None).await else {
                eprintln!("cloud: answer was not created");
                return;
            };
            if peer.set_local_description(answer.clone()).await.is_err() {
                eprintln!("cloud: answer was not applied");
                return;
            }
            eprintln!("cloud: answer sent");
            let _ = outbound.send(Outbound::Answer(answer.sdp));
        }
        "answer" => {
            let Some(sdp) = value.sdp else {
                return;
            };
            if let Ok(remote) = RTCSessionDescription::answer(sdp) {
                if peer.set_remote_description(remote).await.is_ok() {
                    eprintln!("cloud: remote answer applied");
                    flush_ice(peer, pending_ice).await;
                }
            }
        }
        "candidate" | "ice-candidate" => {
            let Some(candidate) = value.candidate else {
                return;
            };
            let init = RTCIceCandidateInit {
                candidate,
                sdp_mid: value.sdp_mid,
                sdp_mline_index: value.sdp_mline_index,
                username_fragment: None,
            };
            if peer.remote_description().await.is_none() {
                pending_ice.lock().await.push(init);
            } else {
                let _ = peer.add_ice_candidate(init).await;
            }
        }
        _ => {
            if let Some(nested) = value.data.as_ref().and_then(|data| data.as_str()) {
                Box::pin(handle_signal(peer, outbound, target, nested, port, opened, pending_ice)).await;
            }
        }
    }
}

async fn flush_ice(peer: &RTCPeerConnection, pending_ice: &tokio::sync::Mutex<Vec<RTCIceCandidateInit>>) {
    let queued = std::mem::take(&mut *pending_ice.lock().await);
    for init in queued {
        let _ = peer.add_ice_candidate(init).await;
    }
}

fn note_notification(text: &str, parsed: usize) {
    let value = serde_json::from_str::<Value>(text).ok();
    let keys = value
        .as_ref()
        .and_then(|item| item.as_object())
        .map(|map| map.keys().cloned().collect::<Vec<_>>().join(","))
        .unwrap_or_else(|| "-".to_string());
    let kind = value.as_ref().and_then(|item| {
        item.get("type")
            .or_else(|| item.get("messageType"))
            .or_else(|| item.get("eventType"))
            .and_then(|item| item.as_str())
    });
    let kind = match kind {
        Some("offer" | "answer" | "ice-candidate" | "candidate" | "device-list-changed" | "token_refresh_response") => kind.unwrap_or("-"),
        _ => "-",
    };
    let sub = short_token(value.as_ref().and_then(|item| item.get("subPath")).and_then(|item| item.as_str()));
    let payload = payload_class(value.as_ref().and_then(|item| item.get("payload")));
    eprintln!("cloud: nrt bytes={} keys={keys} type={kind} sub={sub} payload={payload} signals={parsed}", text.len());
}

fn short_token(value: Option<&str>) -> String {
    match value {
        Some(text) if is_signal_kind(text) || text == "device-list-changed" || text == "token_refresh_response" => text.to_string(),
        Some(text) => format!("len{}", text.chars().count()),
        None => "-".to_string(),
    }
}

fn payload_class(value: Option<&Value>) -> String {
    match value {
        Some(Value::String(text)) => format!("str{} media={} ufrag={}", text.len(), sdp_has_media(text), ufrag_class(text)),
        Some(Value::Object(_)) => "obj".to_string(),
        Some(Value::Array(items)) => format!("arr{}", items.len()),
        Some(_) => "other".to_string(),
        None => "-".to_string(),
    }
}

fn is_signal_kind(kind: &str) -> bool {
    matches!(kind, "offer" | "answer" | "candidate" | "ice-candidate")
}

fn sdp_has_media(sdp: &str) -> bool {
    sdp.lines().any(|line| line.trim_end_matches('\r').starts_with("m="))
}

fn ufrag_class(sdp: &str) -> &'static str {
    let mut empty = false;
    let mut set = false;
    for line in sdp.lines() {
        let line = line.trim_end_matches('\r');
        let Some(rest) = line.strip_prefix("a=ice-ufrag:") else {
            continue;
        };
        if rest.is_empty() {
            empty = true;
        } else {
            set = true;
        }
    }
    if set {
        "set"
    } else if empty {
        "empty"
    } else {
        "-"
    }
}

/// A wake-up echo has an empty ice-ufrag and no media line. Applying it fails and hides a real offer.
fn usable_offer(sdp: &str) -> bool {
    sdp_has_media(sdp) && ufrag_class(sdp) == "set"
}

#[derive(Deserialize)]
struct Signal {
    #[serde(rename = "type")]
    kind: Option<String>,
    #[serde(rename = "eventType")]
    event_type: Option<String>,
    sdp: Option<String>,
    candidate: Option<String>,
    #[serde(rename = "sdpMid")]
    sdp_mid: Option<String>,
    #[serde(rename = "sdpMLineIndex")]
    sdp_mline_index: Option<u16>,
    #[serde(rename = "deviceId")]
    device_id: Option<String>,
    data: Option<Value>,
}

const MESH_USER_AGENT: &str = "mesh-sdk-go/0.3.455";
const SDK_VERSION: &str = "v0.3.455";
/// Offer with an empty ice-ufrag. The phone treats that as a request to start its own tunnel.
const WAKE_OFFER: &str = "v=0\r\no=- 0 0 IN IP4 0.0.0.0\r\ns=-\r\nt=0 0\r\na=ice-ufrag:\r\n";

enum Outbound {
    Offer(String),
    Answer(String),
    Candidate {
        candidate: String,
        sdp_mid: Option<String>,
        sdp_mline_index: Option<u16>,
    },
}

/// Broker device ids are base64url. The Ready For id is standard base64 and its `/` breaks the path.
pub(crate) fn mesh_device_id(real_id: &str) -> String {
    if let Some(bytes) = crate::protocol::b64_decode(real_id) {
        if !bytes.is_empty() {
            return URL_SAFE_NO_PAD.encode(bytes);
        }
    }
    real_id.to_string()
}

fn http_client() -> reqwest::Client {
    reqwest::Client::builder()
        .user_agent(MESH_USER_AGENT)
        .build()
        .unwrap_or_else(|_| reqwest::Client::new())
}

async fn nrt_channels(base: &str) -> Result<Vec<String>, ()> {
    let response = http_client().get(policy_url(base)).send().await.map_err(|_| ())?;
    if !response.status().is_success() {
        return Err(());
    }
    let body = response.text().await.map_err(|_| ())?;
    Ok(channels_from_policy(&body))
}

async fn send_signal(token: &str, base: &str, target: &str, source: &str, item: &Outbound) -> Result<(), ()> {
    let (kind, payload) = match item {
        Outbound::Offer(sdp) | Outbound::Answer(sdp) => {
            let kind = if matches!(item, Outbound::Offer(_)) { "offer" } else { "answer" };
            (kind, sdp.clone())
        }
        Outbound::Candidate {
            candidate,
            sdp_mid: _,
            sdp_mline_index: _,
        } => ("ice-candidate", candidate.clone()),
    };
    if payload.is_empty() {
        return Err(());
    }
    let service = service_base(base);
    let response = http_client()
        .post(format!(
            "{service}/devices/{}/messages/type/{kind}",
            path_segment(target)
        ))
        .bearer_auth(token)
        .json(&json!({
            "payload": payload,
            "sourceDeviceId": source,
            "sdkVersion": SDK_VERSION,
        }))
        .send()
        .await
        .map_err(|_| ())?;
    if response.status().is_success() {
        Ok(())
    } else {
        Err(())
    }
}

/// Pull offer, answer, and ICE lines out of a notification body. Values are not logged.
fn normalize_signal(text: &str) -> Vec<String> {
    let Ok(value) = serde_json::from_str::<Value>(text) else {
        return Vec::new();
    };
    let mut out = Vec::new();
    walk_signal(&value, None, 0, &mut out);
    out
}

fn walk_signal(value: &Value, inherited: Option<&str>, depth: u8, out: &mut Vec<String>) {
    if depth > 6 {
        return;
    }
    if let Value::Array(items) = value {
        for item in items {
            walk_signal(item, inherited, depth + 1, out);
        }
        return;
    }
    let Value::Object(map) = value else {
        if let Value::String(text) = value {
            if let Ok(nested) = serde_json::from_str::<Value>(text) {
                walk_signal(&nested, inherited, depth + 1, out);
            }
        }
        return;
    };
    let tagged = map
        .get("type")
        .or_else(|| map.get("eventType"))
        .or_else(|| map.get("messageType"))
        .and_then(|value| value.as_str())
        .filter(|kind| is_signal_kind(kind) || *kind == "device-list-changed");
    let sub = map.get("subPath").and_then(|value| value.as_str()).filter(|kind| is_signal_kind(kind));
    let kind = tagged
        .or(sub)
        .map(str::to_string)
        .or_else(|| inherited.map(str::to_string));
    if kind.as_deref() == Some("device-list-changed") {
        return;
    }
    if let Some(kind) = kind.as_deref() {
        if matches!(kind, "offer" | "answer" | "candidate" | "ice-candidate") {
            let sdp = map
                .get("sdp")
                .and_then(|value| value.as_str())
                .map(str::to_string)
                .or_else(|| {
                    if matches!(kind, "offer" | "answer") {
                        map.get("payload")
                            .and_then(|value| value.as_str())
                            .filter(|text| text.contains("v="))
                            .map(str::to_string)
                    } else {
                        None
                    }
                });
            let candidate = map
                .get("candidate")
                .and_then(|value| value.as_str())
                .map(str::to_string)
                .or_else(|| {
                    if matches!(kind, "candidate" | "ice-candidate") {
                        map.get("payload").and_then(|value| value.as_str()).map(str::to_string)
                    } else {
                        None
                    }
                });
            let sdp_mid = map.get("sdpMid").and_then(|value| value.as_str());
            let line = map.get("sdpMLineIndex").and_then(|value| value.as_u64()).and_then(|value| u16::try_from(value).ok());
            if sdp.is_some() || candidate.is_some() {
                out.push(
                    json!({
                        "type": kind,
                        "sdp": sdp,
                        "candidate": candidate,
                        "sdpMid": sdp_mid,
                        "sdpMLineIndex": line,
                    })
                    .to_string(),
                );
            }
        }
    }
    for key in ["payload", "data", "message", "messages", "content", "body", "notification"] {
        if let Some(child) = map.get(key) {
            if child.is_object() || child.is_string() || child.is_array() {
                walk_signal(child, kind.as_deref(), depth + 1, out);
            }
        }
    }
}

async fn poll_inbox(
    token: &str,
    base: &str,
    mesh_id: &str,
    peer: &RTCPeerConnection,
    outbound: &mpsc::UnboundedSender<Outbound>,
    target: &str,
    port: u16,
    opened: Arc<tokio::sync::Notify>,
    pending_ice: &tokio::sync::Mutex<Vec<RTCIceCandidateInit>>,
) {
    let url = format!("{}/devices/{}/messages", service_base(base), path_segment(mesh_id));
    let Ok(response) = http_client()
        .get(url)
        .bearer_auth(token)
        .timeout(Duration::from_secs(8))
        .send()
        .await
    else {
        eprintln!("cloud: poll unreachable");
        return;
    };
    let status = response.status();
    let Ok(body) = response.text().await else {
        eprintln!("cloud: poll status={}", status.as_u16());
        return;
    };
    let normalized = normalize_signal(&body);
    eprintln!("cloud: poll status={} bytes={} signals={}", status.as_u16(), body.len(), normalized.len());
    for item in normalized {
        handle_signal(peer, outbound, target, &item, port, opened.clone(), pending_ice).await;
    }
}

async fn register_device(token: &str, base: &str, name: &str, real_id: &str, mesh_id: &str) -> Result<(), String> {
    let base = service_base(base);
    let body = json!({
        "deviceId": mesh_id,
        "realDeviceId": real_id,
        "deviceName": name,
        "os": 6,
        "type": 4,
        "scType": 2,
    });
    let response = http_client()
        .post(format!("{base}/devices/{}", path_segment(mesh_id)))
        .bearer_auth(token)
        .json(&body)
        .send()
        .await
        .map_err(|_| "Could not register this Mac with the cloud broker.".to_string())?;
    if response.status().is_success() || response.status().as_u16() == 409 {
        Ok(())
    } else {
        Err("Could not register this Mac with the cloud broker.".to_string())
    }
}

async fn turn_servers(token: &str, base: &str) -> Result<Vec<RTCIceServer>, ()> {
    let base = service_base(base);
    let response = http_client()
        .post(format!("{base}/turn-credentials"))
        .bearer_auth(token)
        .json(&json!({"ttl": 3600}))
        .send()
        .await
        .map_err(|_| ())?;
    if !response.status().is_success() {
        return Err(());
    }
    let body = response.text().await.map_err(|_| ())?;
    Ok(parse_ice(&body))
}

fn parse_ice(body: &str) -> Vec<RTCIceServer> {
    let Ok(json) = serde_json::from_str::<Value>(body) else {
        return Vec::new();
    };
    let mut servers = Vec::new();
    collect_ice(&json, &mut servers);
    if let Some(nested) = json.get("turn_credentials") {
        collect_ice(nested, &mut servers);
    }
    if let Some(nested) = json.get("iceServers") {
        collect_ice(nested, &mut servers);
    }
    servers
}

fn collect_ice(value: &Value, servers: &mut Vec<RTCIceServer>) {
    let items: Vec<&Value> = if let Some(list) = value.as_array() {
        list.iter().collect()
    } else if value.get("urls").is_some() || value.get("uris").is_some() {
        vec![value]
    } else {
        Vec::new()
    };
    for item in items {
        let urls = item
            .get("urls")
            .or_else(|| item.get("uris"))
            .map(|entry| match entry {
                Value::String(url) => vec![url.clone()],
                Value::Array(list) => list.iter().filter_map(|url| url.as_str().map(str::to_string)).collect(),
                _ => Vec::new(),
            })
            .unwrap_or_default();
        if urls.is_empty() {
            continue;
        }
        let username = item.get("username").and_then(|value| value.as_str()).unwrap_or("").to_string();
        let credential = item
            .get("credential")
            .or_else(|| item.get("password"))
            .and_then(|value| value.as_str())
            .unwrap_or("")
            .to_string();
        if urls.iter().any(|url| url.starts_with("turn")) && (username.is_empty() || credential.is_empty()) {
            continue;
        }
        servers.push(RTCIceServer {
            urls,
            username,
            credential,
            credential_type: RTCIceCredentialType::Password,
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mesh_id_is_url_safe() {
        let ready = crate::protocol::b64(&[0u8; 16]);
        let mesh = mesh_device_id(&ready);
        assert_eq!(mesh.len(), 22);
        assert!(!mesh.contains('/'));
        assert!(!mesh.contains('+'));
        assert!(!mesh.contains('='));
    }

    #[test]
    fn notification_body_yields_the_sdp() {
        let messages = normalize_signal(
            r#"{"type":"answer","payload":"v=0\r\no=- 1 1 IN IP4 127.0.0.1\r\n","sourceDeviceId":"phone-1"}"#,
        );
        assert_eq!(messages.len(), 1);
        let parsed: Value = serde_json::from_str(&messages[0]).unwrap();
        assert_eq!(parsed["type"], "answer");
        assert!(parsed["sdp"].as_str().unwrap().starts_with("v=0"));
        assert!(normalize_signal(r#"{"type":"device-list-changed"}"#).is_empty());
    }

    #[test]
    fn turn_password_is_marked_as_a_password() {
        let servers = parse_ice(
            r#"{"turn_credentials":{"username":"user","password":"secret","uris":["turns:example.com:443?transport=tcp","stuns:example.com:5349"]}}"#,
        );
        assert_eq!(servers.len(), 1);
        assert_eq!(servers[0].credential_type, RTCIceCredentialType::Password);
        assert_eq!(servers[0].username, "user");
        assert_eq!(servers[0].credential, "secret");
        assert_eq!(servers[0].urls.len(), 2);
    }

    #[test]
    fn wake_offer_has_an_empty_ufrag() {
        assert!(WAKE_OFFER.contains("a=ice-ufrag:\r\n"));
        assert!(!WAKE_OFFER.contains("a=ice-ufrag:x"));
        assert!(!usable_offer(WAKE_OFFER));
    }

    #[test]
    fn subpath_offer_is_a_signal() {
        let messages = normalize_signal(r#"{"subPath":"offer","payload":"v=0\r\nm=application 9 UDP/DTLS/SCTP webrtc-datachannel\r\na=ice-ufrag:abc\r\n"}"#);
        assert_eq!(messages.len(), 1);
        let parsed: Value = serde_json::from_str(&messages[0]).unwrap();
        assert_eq!(parsed["type"], "offer");
        assert!(usable_offer(parsed["sdp"].as_str().unwrap()));
    }
}

fn path_segment(value: &str) -> String {
    let mut out = String::new();
    for byte in value.bytes() {
        match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => out.push(byte as char),
            _ => out.push_str(&format!("%{byte:02X}")),
        }
    }
    out
}
