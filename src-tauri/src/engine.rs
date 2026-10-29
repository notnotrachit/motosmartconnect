//! Pairing session: QR token, control socket, discovery invites, and mDNS.

use crate::desk::{self, Desk, Effect, FileView};
use crate::mirror::Mirror;
use crate::fex;
use crate::nfs;
use crate::net::{self, phone_client_config};
use crate::protocol::{self, OpenedFrame, TOKEN_TTL_SECS};
use crate::sms::{self, Sms};
use crate::store::{self, Identity, PhoneRecord};
use anyhow::{Context, Result};
use rustls::pki_types::ServerName;
use futures_util::{SinkExt, StreamExt};
use mdns_sd::{ServiceDaemon, ServiceEvent, ServiceInfo};
use serde::Serialize;
use serde_json::{json, Value};
use std::collections::{HashMap, VecDeque};
use std::net::{IpAddr, Ipv4Addr, SocketAddr};
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex, MutexGuard};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};
use tauri::{AppHandle, Emitter};
use tokio::io::{AsyncRead, AsyncWrite};
use tokio::net::TcpStream;
use tokio::sync::{mpsc, Notify};
use tokio_rustls::TlsConnector;
use tokio_tungstenite::tungstenite::protocol::WebSocketConfig;
use tokio_tungstenite::tungstenite::Message;
use tokio_tungstenite::{accept_hdr_async_with_config, client_async};

const MDNS_TYPE: &str = "_readyformdns._tcp.local.";
const INVITE_GAP: Duration = Duration::from_secs(20);

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PhoneView {
    pub real_id: String,
    pub name: String,
    pub connected: bool,
    pub user_disconnected: bool,
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Snapshot {
    pub device_name: String,
    pub real_device_id: String,
    pub fingerprint_b64: String,
    pub port: u16,
    pub ips: Vec<String>,
    pub qr_svg: String,
    pub qr_expires_in: u64,
    pub pairing: bool,
    pub phones: Vec<PhoneView>,
    pub log: Vec<String>,
    pub listening: bool,
    pub error: Option<String>,
    pub notifications: Vec<desk::NoticeView>,
    pub clipboard_ready: bool,
    pub apps: Vec<desk::AppView>,
    pub icons_revision: u64,
    pub files: Vec<desk::FileView>,
    pub sessions: Vec<desk::SessionView>,
    pub stream: Option<desk::SessionView>,
    pub cross: desk::CrossView,
    pub lan_ip: String,
    pub messages: sms::SmsView,
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct IconPack {
    pub revision: u64,
    pub icons: HashMap<String, String>,
}

struct LiveToken {
    value: String,
    expires: Instant,
    /// True for a QR the user asked for. Reconnect invites are not stored here.
    pairing: bool,
}

struct Inner {
    identity: Identity,
    phones: Vec<PhoneRecord>,
    pending_keys: HashMap<String, String>,
    pending_names: HashMap<String, String>,
    tokens: Vec<LiveToken>,
    nonce: [u8; 16],
    port: u16,
    ips: Vec<String>,
    qr_svg: String,
    qr_expires: Option<Instant>,
    pairing: bool,
    logs: VecDeque<String>,
    listening: bool,
    error: Option<String>,
    senders: HashMap<String, (u64, mpsc::UnboundedSender<Message>)>,
    last_invite: HashMap<String, Instant>,
    mdns_fullname: Option<String>,
    desk: Desk,
    peers: HashMap<String, IpAddr>,
    session_peers: HashMap<u64, IpAddr>,
    session_locals: HashMap<u64, IpAddr>,
    wallpaper_md5: String,
    wallpaper_url: String,
    wallpaper_asked: bool,
    reported_power: Option<crate::power::HostPower>,
    sms: Sms,
}

struct Shared {
    inner: Mutex<Inner>,
    app: AppHandle,
    next_msg: AtomicU64,
    next_session: AtomicU64,
    mdns: Mutex<Option<ServiceDaemon>>,
    client_tls: Mutex<Option<Arc<rustls::ClientConfig>>>,
    mirror: Arc<Mirror>,
    pair_wake: Notify,
    share_ready: Notify,
    page_offers: Mutex<Vec<PageOffer>>,
}

struct PageOffer {
    session_id: String,
    sdp: String,
    quiet: bool,
    pkg: String,
}

#[derive(Clone)]
pub struct Engine {
    shared: Arc<Shared>,
}

enum Step {
    Continue,
    Drop,
}

impl Engine {
    pub fn new(app: AppHandle) -> Result<Self> {
        net::install_crypto();
        let (identity, phones) = store::load().context("loading the saved identity")?;
        let nonce = protocol::fresh_nonce(&identity.real_bytes);
        let engine = Self {
            shared: Arc::new(Shared {
                inner: Mutex::new(Inner {
                    identity,
                    phones,
                    pending_keys: HashMap::new(),
                    pending_names: HashMap::new(),
                    tokens: Vec::new(),
                    nonce,
                    port: 0,
                    ips: Vec::new(),
                    qr_svg: String::new(),
                    qr_expires: None,
                    pairing: false,
                    logs: VecDeque::new(),
                    listening: false,
                    error: None,
                    senders: HashMap::new(),
                    last_invite: HashMap::new(),
                    mdns_fullname: None,
                    desk: Desk::new(),
                    peers: HashMap::new(),
                    session_peers: HashMap::new(),
                    session_locals: HashMap::new(),
                    wallpaper_md5: String::new(),
                    wallpaper_url: String::new(),
                    wallpaper_asked: false,
                    reported_power: None,
                    sms: Sms::new(),
                }),
                app,
                next_msg: AtomicU64::new(1),
                next_session: AtomicU64::new(1),
                mdns: Mutex::new(None),
                client_tls: Mutex::new(None),
                mirror: Mirror::new(),
                pair_wake: Notify::new(),
                share_ready: Notify::new(),
                page_offers: Mutex::new(Vec::new()),
            }),
        };
        engine.note("Ready to listen.");
        Ok(engine)
    }

    pub async fn boot(&self) {
        net::install_crypto();
        let (listener, port) = match net::bind_listener().await {
            Ok(bound) => bound,
            Err(err) => {
                self.fail(format!("Could not open a port: {err}"));
                return;
            }
        };
        let (cert_pem, key_pem) = {
            let guard = self.lock();
            (guard.identity.cert_pem.clone(), guard.identity.key_pem.clone())
        };
        let server = match net::server_config(&cert_pem, &key_pem) {
            Ok(config) => config,
            Err(err) => {
                self.fail(format!("Could not load the certificate: {err}"));
                return;
            }
        };
        {
            let mut guard = self.lock();
            guard.port = port;
            guard.ips = net::local_ipv4s();
            guard.error = None;
        }
        let client = Arc::new(phone_client_config());
        *self.shared.client_tls.lock().unwrap_or_else(|err| err.into_inner()) = Some(client);
        self.start_mdns();
        {
            self.lock().listening = true;
        }
        let only_loopback = self.lock().ips.iter().all(|ip| ip.starts_with("127."));
        if only_loopback {
            self.note("No local network address yet. The code will work after this Mac joins Wi-Fi.");
        }
        self.note(format!("Listening on port {port}."));
        let start_pairing = {
            let guard = self.lock();
            guard.phones.is_empty() || guard.pairing
        };
        if start_pairing {
            self.begin_pairing();
        } else {
            self.note("Pairing code is off. A saved phone can still reconnect.");
            self.publish();
        }

        let acceptor = tokio_rustls::TlsAcceptor::from(server);
        let engine = self.clone();
        tokio::spawn(async move {
            loop {
                let (tcp, peer) = match listener.accept().await {
                    Ok(accepted) => accepted,
                    Err(err) => {
                        engine.note(format!("Accept failed: {err}"));
                        break;
                    }
                };
                let local = tcp.local_addr().ok().and_then(media_ipv4);
                let acceptor = acceptor.clone();
                let engine = engine.clone();
                tokio::spawn(async move {
                    let tls = match acceptor.accept(tcp).await {
                        Ok(stream) => stream,
                        Err(err) => {
                            engine.note(format!("{peer} stopped during TLS ({err})."));
                            return;
                        }
                    };
                    engine.dispatch(tls, peer, local).await;
                });
            }
        });

        let engine = self.clone();
        tokio::spawn(async move {
            loop {
                let notified = engine.shared.pair_wake.notified();
                tokio::pin!(notified);
                if !engine.lock().pairing {
                    notified.await;
                    continue;
                }
                let wait = engine.token_wait();
                tokio::select! {
                    _ = notified => {}
                    _ = tokio::time::sleep(wait) => {
                        if engine.lock().pairing {
                            engine.rotate_token();
                            engine.refresh_mdns();
                            engine.publish();
                        }
                    }
                }
            }
        });

        let engine = self.clone();
        tokio::spawn(async move {
            loop {
                tokio::time::sleep(Duration::from_secs(10)).await;
                engine.heartbeat();
            }
        });

        let engine = self.clone();
        tokio::spawn(async move {
            loop {
                tokio::time::sleep(Duration::from_millis(900)).await;
                engine.poll_clipboard();
            }
        });
    }

    pub fn wallpaper(&self) -> String {
        self.lock().wallpaper_url.clone()
    }

    /// The picture is a base64 JPEG. The same md5 is skipped.
    /// The bytes are never written to the activity log.
    /// Windows asks with service 25 command 4 until `wallpaperState` is 7.
    /// That is the state where the phone is allowed to send the picture.
    fn ask_for_wallpaper(&self, data: &Value, tx: &mpsc::UnboundedSender<Message>) {
        let state = data.get("wallpaperState").and_then(Value::as_i64);
        let already = {
            let guard = self.lock();
            !guard.wallpaper_url.is_empty() || guard.wallpaper_asked
        };
        if already {
            return;
        }
        if state == Some(7) {
            self.note("The phone allows its wallpaper. Waiting for the picture.");
            self.lock().wallpaper_asked = true;
            return;
        }
        let text = {
            let guard = self.lock();
            protocol::wallpaper_request(&protocol::b64(&guard.nonce), self.msg_id())
        };
        if tx.send(Message::Text(text.into())).is_err() {
            return;
        }
        self.lock().wallpaper_asked = true;
        match state {
            Some(state) => self.note(format!("Asked the phone for its wallpaper (state {state}). Allow it on the phone if it asks.")),
            None => self.note("Asked the phone for its wallpaper. Allow it on the phone if it asks.".to_string()),
        }
    }

    fn store_wallpaper(&self, raw: &str, md5: &str) {
        let md5 = md5.trim();
        if raw.len() < 32 {
            return;
        }
        if !md5.is_empty() && self.lock().wallpaper_md5 == md5 {
            return;
        }
        let Some(url) = wallpaper_data_url(raw) else {
            self.note("The phone sent a wallpaper this Mac could not show.");
            return;
        };
        {
            let mut guard = self.lock();
            guard.wallpaper_md5 = md5.to_string();
            guard.wallpaper_url = url.clone();
        }
        let _ = self.shared.app.emit("wallpaper", &url);
        self.note("The phone's wallpaper is on the screen.");
    }

    pub fn snapshot(&self) -> Snapshot {
        let guard = self.lock();
        let qr_expires_in = guard
            .qr_expires
            .map(|deadline| deadline.saturating_duration_since(Instant::now()).as_secs())
            .unwrap_or(0);
        Snapshot {
            device_name: guard.identity.device_name.clone(),
            real_device_id: guard.identity.real_id.clone(),
            fingerprint_b64: guard.identity.fp_b64.clone(),
            port: guard.port,
            ips: guard.ips.clone(),
            qr_svg: guard.qr_svg.clone(),
            qr_expires_in,
            pairing: guard.pairing,
            phones: guard
                .phones
                .iter()
                .map(|phone| PhoneView {
                    real_id: phone.real_id.clone(),
                    name: phone.name.clone(),
                    connected: guard.senders.contains_key(&phone.real_id),
                    user_disconnected: phone.user_disconnected,
                })
                .collect(),
            log: guard.logs.iter().cloned().collect(),
            listening: guard.listening,
            error: guard.error.clone(),
            notifications: guard.desk.notifications(),
            clipboard_ready: guard.desk.clipboard_ready,
            apps: guard.desk.apps(),
            icons_revision: guard.desk.icons_revision,
            files: guard.desk.files(),
            sessions: guard.desk.sessions(),
            stream: guard.desk.visible(),
            cross: guard.desk.cross(),
            lan_ip: guard.ips.iter().find(|ip| *ip != "127.0.0.1").cloned().unwrap_or_default(),
            messages: guard.sms.view(),
        }
    }

    pub fn set_name(&self, name: &str) -> Result<(), String> {
        let name = name.trim();
        if name.is_empty() || name.chars().count() > 40 {
            return Err("Use a name of 1 to 40 characters.".into());
        }
        {
            let mut guard = self.lock();
            guard.identity.device_name = name.to_string();
            let identity = guard.identity.clone();
            let phones = guard.phones.clone();
            drop(guard);
            store::save(&identity, &phones).map_err(|err| err.to_string())?;
        }
        if self.lock().pairing {
            self.rotate_token();
            self.note("New code is ready.");
        }
        self.refresh_mdns();
        self.note(format!("Name is now {name}."));
        self.publish();
        Ok(())
    }

    /// Mint a pairing code. A saved phone can reconnect without one.
    pub fn begin_pairing(&self) {
        let already = self.lock().pairing;
        self.lock().pairing = true;
        if self.lock().port == 0 {
            return;
        }
        self.rotate_token();
        self.refresh_mdns();
        self.note(if already { "New code is ready." } else { "Pairing code is on." });
        self.publish();
        self.shared.pair_wake.notify_waiters();
    }

    /// Drop every pairing code. The listener and saved-phone discovery stay up.
    pub fn end_pairing(&self) {
        let changed = {
            let mut guard = self.lock();
            let changed = guard.pairing || !guard.qr_svg.is_empty() || guard.tokens.iter().any(|token| token.pairing);
            guard.pairing = false;
            guard.tokens.retain(|token| !token.pairing);
            guard.qr_svg.clear();
            guard.qr_expires = None;
            changed
        };
        self.shared.pair_wake.notify_waiters();
        if changed {
            self.note("Pairing code is off.");
            self.publish();
        }
    }

    pub fn refresh_qr(&self) {
        self.begin_pairing();
    }

    pub async fn unpair(&self, real_id: &str) {
        let sender = self.lock().senders.get(real_id).map(|(_, tx)| tx.clone());
        if sender.is_some() {
            let text = {
                let guard = self.lock();
                protocol::delete_message(&protocol::b64(&guard.nonce), real_id, self.msg_id())
            };
            if let Some(tx) = sender {
                let _ = tx.send(Message::Text(text.into()));
                tokio::time::sleep(Duration::from_millis(250)).await;
            }
        }
        let name = {
            let mut guard = self.lock();
            let name = guard
                .phones
                .iter()
                .find(|phone| phone.real_id == real_id)
                .map(|phone| phone.name.clone())
                .unwrap_or_else(|| "Phone".into());
            guard.phones.retain(|phone| phone.real_id != real_id);
            guard.pending_keys.remove(real_id);
            guard.senders.remove(real_id);
            if guard.phones.is_empty() {
                guard.wallpaper_md5.clear();
                guard.wallpaper_url.clear();
            }
            guard.sms.clear();
            let identity = guard.identity.clone();
            let phones = guard.phones.clone();
            drop(guard);
            if let Err(err) = store::save(&identity, &phones) {
                self.note(format!("Could not update the saved phones: {err}"));
            }
            name
        };
        self.note(format!("Forgot {name} on this Mac."));
        if self.lock().phones.is_empty() {
            let _ = self.shared.app.emit("wallpaper", "");
            self.begin_pairing();
        } else {
            self.publish();
        }
    }

    async fn dispatch<S>(&self, stream: S, peer: SocketAddr, local: Option<IpAddr>)
    where
        S: AsyncRead + AsyncWrite + Unpin + Send + 'static,
    {
        let mut path = String::new();
        let ws_config = WebSocketConfig::default()
            .max_message_size(Some(32 * 1024 * 1024))
            .max_frame_size(Some(32 * 1024 * 1024));
        let websocket = match accept_hdr_async_with_config(
            stream,
            |request: &tokio_tungstenite::tungstenite::handshake::server::Request, response| {
                path = request.uri().path().to_string();
                Ok(response)
            },
            Some(ws_config),
        )
        .await
        {
            Ok(socket) => socket,
            Err(err) => {
                self.note(format!("{peer} did not complete the socket ({err})."));
                return;
            }
        };
        if path.contains("mdns") {
            self.handle_discovery(websocket, peer).await;
        } else {
            self.handle_control(websocket, peer, local).await;
        }
    }

    async fn handle_control<S>(&self, websocket: tokio_tungstenite::WebSocketStream<S>, peer: SocketAddr, local: Option<IpAddr>)
    where
        S: AsyncRead + AsyncWrite + Unpin + Send + 'static,
    {
        let (mut sink, mut stream) = websocket.split();
        let (tx, mut rx) = mpsc::unbounded_channel::<Message>();
        let writer = tokio::spawn(async move {
            while let Some(message) = rx.recv().await {
                if sink.send(message).await.is_err() {
                    break;
                }
            }
        });
        let session = self.shared.next_session.fetch_add(1, Ordering::Relaxed);
        {
            let mut guard = self.lock();
            guard.session_peers.insert(session, peer.ip());
            if let Some(ip) = local {
                guard.session_locals.insert(session, ip);
            }
        }
        let exchange_sent = Arc::new(AtomicBool::new(false));
        let mut phone_id: Option<String> = None;

        while let Some(incoming) = stream.next().await {
            let message = match incoming {
                Ok(message) => message,
                Err(_) => break,
            };
            match message {
                Message::Text(text) => {
                    let step = self.on_control_text(&text, &tx, session, &exchange_sent, &mut phone_id);
                    if matches!(step, Step::Drop) {
                        break;
                    }
                }
                Message::Binary(bytes) => {
                    if let Ok(text) = std::str::from_utf8(&bytes) {
                        let step = self.on_control_text(text, &tx, session, &exchange_sent, &mut phone_id);
                        if matches!(step, Step::Drop) {
                            break;
                        }
                    }
                }
                Message::Ping(payload) => {
                    let _ = tx.send(Message::Pong(payload));
                }
                Message::Close(_) => break,
                _ => {}
            }
        }

        if let Some(real_id) = &phone_id {
            let mut guard = self.lock();
            if guard.senders.get(real_id).map(|(id, _)| *id) == Some(session) {
                guard.senders.remove(real_id);
                let closed = guard.desk.on_connected();
                drop(guard);
                self.apply_effects(closed, None);
            } else {
                drop(guard);
            }
            let name = {
                let guard = self.lock();
                guard.phones.iter().find(|phone| &phone.real_id == real_id).map(|phone| phone.name.clone())
            };
            if let Some(name) = name {
                self.note(format!("{name} disconnected."));
                self.publish();
            }
        }
        {
            let mut guard = self.lock();
            guard.session_peers.remove(&session);
            guard.session_locals.remove(&session);
        }
        drop(tx);
        let _ = writer.await;
        let _ = peer;
    }

    fn on_control_text(
        &self,
        text: &str,
        tx: &mpsc::UnboundedSender<Message>,
        session: u64,
        exchange_sent: &Arc<AtomicBool>,
        phone_id: &mut Option<String>,
    ) -> Step {
        let owned;
        let text = if text.contains("\"mediaData\"") {
            owned = sms::strip_media_data(text);
            owned.as_str()
        } else {
            text
        };
        let message: Value = match serde_json::from_str(&protocol::quote_wide_integers(text)) {
            Ok(value) => value,
            Err(_) => {
                if text.len() > 2000 {
                    self.note(format!("A phone message of {} bytes could not be read.", text.len()));
                }
                return Step::Continue;
            }
        };
        let service = message.get("serviceId").and_then(Value::as_i64).unwrap_or(0);
        let src = message.get("srcServiceId").and_then(Value::as_i64).unwrap_or(service);
        let data = protocol::data_object(&message).unwrap_or_else(|| message.clone());
        let cmd = protocol::json_cmd(&data).or_else(|| protocol::json_cmd(&message)).unwrap_or(-1);
        if let Some((raw, md5)) = find_wallpaper(&message).or_else(|| find_wallpaper(&data)) {
            self.store_wallpaper(&raw, &md5);
        } else if service == 25 && cmd == 1 {
            self.ask_for_wallpaper(&data, tx);
        } else if service == 25 && text.len() > 4000 {
            self.note(format!("Phone status command {cmd} had no picture."));
        }
        let routed = if service == 26 || src == 26 { 26 } else { service };
        if is_feature(routed) {
            let effects = self.lock().desk.handle(routed, &data);
            self.apply_effects(effects, Some(tx));
            return Step::Continue;
        }

        if service == 204 || (service == 100 && cmd == 1018) {
            self.on_sms(service, cmd, &data);
            return Step::Continue;
        }

        if text.len() > 4000 {
            self.note(format!("Service {service} command {cmd} ({len} bytes).", len = text.len()));
        }
        if service == 22 && cmd == 1 {
            self.note("Lock requested.");
            self.lock_screen();
            return Step::Continue;
        }
        if service != 0 {
            return Step::Continue;
        }

        match cmd {
            0 | 3 => self.on_hello(&data, tx, session, exchange_sent, phone_id),
            1 => {
                self.on_phone_key(&data, phone_id);
                if let Some(real_id) = phone_id.clone() {
                    self.send_exchange(tx, exchange_sent, &real_id);
                }
                Step::Continue
            }
            6 => {
                if let Some(real_id) = phone_id.clone() {
                    self.mark_user_disconnected(&real_id);
                    self.note("The phone disconnected. This Mac will wait for it to call again.");
                    self.publish();
                }
                Step::Continue
            }
            _ => Step::Continue,
        }
    }

    fn on_hello(
        &self,
        data: &Value,
        tx: &mpsc::UnboundedSender<Message>,
        session: u64,
        exchange_sent: &Arc<AtomicBool>,
        phone_id: &mut Option<String>,
    ) -> Step {
        let device = &data["device"];
        let real_id = device["realDeviceId"].as_str().unwrap_or("").trim().to_string();
        if real_id.is_empty() {
            self.note("A phone hello had no stable id.");
            return Step::Drop;
        }
        let token = data["token"].as_str().unwrap_or("");
        let known = self.lock().phones.iter().any(|phone| phone.real_id == real_id);
        if !self.token_matches(token) && !known {
            self.note("Rejected a connection with an unknown code.");
            return Step::Drop;
        }
        let name = device["deviceName"].as_str().unwrap_or("Phone").trim();
        let name = if name.is_empty() { "Phone" } else { name };
        let phone_key = non_empty_str(device.get("key"));
        let our_key = self.pair_key_for(&real_id);
        {
            let mut guard = self.lock();
            if let Some(phone) = guard.phones.iter_mut().find(|phone| phone.real_id == real_id) {
                phone.name = name.to_string();
                phone.user_disconnected = false;
                if let Some(key) = &phone_key {
                    phone.phone_key = Some(key.clone());
                }
            } else {
                guard.pending_names.insert(real_id.clone(), name.to_string());
                if let Some(key) = phone_key.clone() {
                    guard.pending_keys.insert(format!("phone:{real_id}"), key);
                }
            }
            guard.senders.insert(real_id.clone(), (session, tx.clone()));
        }
        *phone_id = Some(real_id.clone());
        let verify = {
            let guard = self.lock();
            protocol::verify_message(
                &protocol::b64(&guard.nonce),
                &guard.identity.real_id,
                &guard.identity.device_name,
                &guard.identity.sn,
                &our_key,
                self.msg_id(),
            )
        };
        let status = {
            let mut guard = self.lock();
            if let Some(ip) = guard.session_peers.get(&session).copied() {
                guard.peers.insert(real_id.clone(), ip);
            }
            let closed = guard.desk.on_connected();
            guard.sms.on_phone(&real_id);
            drop(guard);
            self.apply_effects(closed, Some(tx));
            let power = crate::power::read();
            let mut guard = self.lock();
            guard.reported_power = Some(power);
            protocol::status_message_for(
                &protocol::b64(&guard.nonce),
                &guard.identity.device_name,
                self.msg_id(),
                guard.desk.cross_active,
                power,
            )
        };
        if tx.send(Message::Text(verify.into())).is_err() || tx.send(Message::Text(status.into())).is_err() {
            return Step::Drop;
        }
        self.lock().wallpaper_asked = false;
        self.note(format!("{name} connected."));
        self.note(crate::power::read().line());
        self.publish();

        let engine = self.clone();
        let tx = tx.clone();
        let flag = exchange_sent.clone();
        let real_id_for_timer = real_id;
        tokio::spawn(async move {
            tokio::time::sleep(Duration::from_secs(2)).await;
            engine.send_exchange(&tx, &flag, &real_id_for_timer);
        });
        Step::Continue
    }

    fn on_phone_key(&self, data: &Value, phone_id: &Option<String>) {
        let Some(real_id) = phone_id else { return };
        let device = &data["device"];
        let Some(key) = non_empty_str(device.get("key")) else { return };
        let mut guard = self.lock();
        if let Some(phone) = guard.phones.iter_mut().find(|phone| &phone.real_id == real_id) {
            phone.phone_key = Some(key);
        } else {
            guard.pending_keys.insert(format!("phone:{real_id}"), key);
        }
    }

    fn send_exchange(&self, tx: &mpsc::UnboundedSender<Message>, flag: &AtomicBool, real_id: &str) {
        if flag.swap(true, Ordering::SeqCst) {
            return;
        }
        let (text, name, our_key, phone_key, is_new) = {
            let guard = self.lock();
            let our_key = guard
                .phones
                .iter()
                .find(|phone| phone.real_id == real_id)
                .map(|phone| phone.our_key.clone())
                .or_else(|| guard.pending_keys.get(real_id).cloned())
                .unwrap_or_else(protocol::mint_pair_key);
            let phone_key = guard
                .phones
                .iter()
                .find(|phone| phone.real_id == real_id)
                .and_then(|phone| phone.phone_key.clone())
                .or_else(|| guard.pending_keys.get(&format!("phone:{real_id}")).cloned());
            let is_new = guard.phones.iter().all(|phone| phone.real_id != real_id);
            let name = guard
                .phones
                .iter()
                .find(|phone| phone.real_id == real_id)
                .map(|phone| phone.name.clone())
                .or_else(|| guard.pending_names.get(real_id).cloned())
                .unwrap_or_else(|| "Phone".into());
            let text = protocol::exchange_message(
                &protocol::b64(&guard.nonce),
                &guard.identity.real_id,
                &guard.identity.device_name,
                &guard.identity.sn,
                &our_key,
                self.msg_id(),
            );
            (text, name, our_key, phone_key, is_new)
        };
        if tx.send(Message::Text(text.into())).is_err() {
            return;
        }
        self.remember_phone(real_id, &name, &our_key, phone_key);
        if is_new {
            self.note(format!("Paired with {name}."));
            self.end_pairing();
        } else {
            self.publish();
        }
    }

    fn remember_phone(&self, real_id: &str, name: &str, our_key: &str, phone_key: Option<String>) {
        let mut guard = self.lock();
        if let Some(phone) = guard.phones.iter_mut().find(|phone| phone.real_id == real_id) {
            if !name.is_empty() {
                phone.name = name.to_string();
            }
            phone.our_key = our_key.to_string();
            if phone_key.is_some() {
                phone.phone_key = phone_key;
            }
            phone.user_disconnected = false;
        } else {
            guard.phones.push(PhoneRecord {
                real_id: real_id.to_string(),
                name: if name.is_empty() { "Phone".into() } else { name.into() },
                our_key: our_key.to_string(),
                phone_key,
                user_disconnected: false,
            });
        }
        guard.pending_keys.remove(real_id);
        guard.pending_keys.remove(&format!("phone:{real_id}"));
        guard.pending_names.remove(real_id);
        let identity = guard.identity.clone();
        let phones = guard.phones.clone();
        drop(guard);
        if let Err(err) = store::save(&identity, &phones) {
            self.note(format!("Could not save the pair: {err}"));
        }
    }

    async fn handle_discovery<S>(&self, mut websocket: tokio_tungstenite::WebSocketStream<S>, peer: SocketAddr)
    where
        S: AsyncRead + AsyncWrite + Unpin,
    {
        self.note(format!("Discovery connection from {peer}."));
        while let Some(incoming) = websocket.next().await {
            let message = match incoming {
                Ok(message) => message,
                Err(_) => break,
            };
            if let Message::Binary(frame) = message {
                if let Some(reply) = self.reply_to_trigger(&frame) {
                    if websocket.send(Message::Binary(reply.into())).await.is_err() {
                        break;
                    }
                }
            } else if matches!(message, Message::Close(_)) {
                break;
            }
        }
    }

    fn reply_to_trigger(&self, frame: &[u8]) -> Option<Vec<u8>> {
        if frame.len() < 16 {
            return None;
        }
        let mut prefix = [0u8; 16];
        prefix.copy_from_slice(&frame[..16]);
        let phone = {
            let guard = self.lock();
            guard.phones.iter().find_map(|phone| {
                let real = protocol::b64_decode(&phone.real_id)?;
                if protocol::nonce_matches(&prefix, &real) {
                    Some(phone.clone())
                } else {
                    None
                }
            })
        }?;
        if frame.get(16) == Some(&3) {
            self.note(format!("{} could not decrypt the pair key.", phone.name));
            return None;
        }
        let plain = match protocol::open_frame(&phone.our_key, frame) {
            Ok(OpenedFrame::Payload(plain)) => plain,
            Ok(OpenedFrame::CannotDecrypt) => match phone.phone_key.as_deref() {
                Some(key) => match protocol::open_frame(key, frame) {
                    Ok(OpenedFrame::Payload(plain)) => plain,
                    _ => {
                        self.note(format!("Could not decrypt a discovery frame from {}.", phone.name));
                        return None;
                    }
                },
                None => return None,
            },
            _ => return None,
        };
        let value: Value = serde_json::from_slice(&plain).unwrap_or(Value::Null);
        if protocol::json_cmd(&value) != Some(0) {
            return None;
        }
        self.note(format!("{} asked to reconnect.", phone.name));
        self.sealed_invite(&phone.our_key)
    }

    fn sealed_invite(&self, key: &str) -> Option<Vec<u8>> {
        // The phone echoes this token, but a saved phone is accepted without it.
        // Keeping it out of `tokens` means a reconnect cannot authorize a new device.
        let token = protocol::mint_token();
        let plain = {
            let guard = self.lock();
            protocol::invite_json(
                &protocol::b64(&guard.nonce),
                &guard.identity.device_name,
                &guard.identity.fp_b64,
                &token,
                guard.port,
                &guard.ips,
            )
        };
        match protocol::seal_frame(key, &self.lock().nonce, plain.as_bytes()) {
            Ok(frame) => Some(frame),
            Err(err) => {
                self.note(format!("Could not seal an invite: {err}"));
                None
            }
        }
    }

    fn start_mdns(&self) {
        match ServiceDaemon::new() {
            Ok(daemon) => {
                *self.mdns_mut() = Some(daemon);
                self.refresh_mdns();
                self.spawn_browse();
            }
            Err(err) => self.note(format!("Local discovery is off ({err}). Scanning the code still works.")),
        }
    }

    fn refresh_mdns(&self) {
        let daemon = {
            let guard = self.mdns_mut();
            guard.clone()
        };
        let Some(daemon) = daemon else { return };
        let (instance, nonce_b64, name, fp_hex, port, ips) = {
            let guard = self.lock();
            (
                mdns_instance(&guard.identity.device_name),
                protocol::b64(&guard.nonce),
                guard.identity.device_name.clone(),
                guard.identity.fp_hex.clone(),
                guard.port,
                guard.ips.clone(),
            )
        };
        if port == 0 {
            return;
        }
        let timestamp = unix_secs().to_string();
        let properties = [
            ("deviceId", nonce_b64.as_str()),
            ("deviceName", name.as_str()),
            ("type", "4"),
            ("os", "6"),
            ("appType", "1"),
            ("cap", "0"),
            ("timestamp", timestamp.as_str()),
            ("fp", fp_hex.as_str()),
        ];
        let addresses: Vec<IpAddr> = ips.iter().filter_map(|ip| ip.parse().ok()).collect();
        let info = match ServiceInfo::new(MDNS_TYPE, &instance, "motosmartconnect.local.", addresses.as_slice(), port, &properties[..]) {
            Ok(info) => info,
            Err(err) => {
                self.note(format!("Could not publish on the local network: {err}"));
                return;
            }
        };
        let fullname = info.get_fullname().to_string();
        if let Some(previous) = self.lock().mdns_fullname.clone() {
            let _ = daemon.unregister(&previous);
        }
        if let Err(err) = daemon.register(info) {
            self.note(format!("Could not publish on the local network: {err}"));
            return;
        }
        self.lock().mdns_fullname = Some(fullname);
    }

    fn spawn_browse(&self) {
        let daemon = self.mdns_mut().clone();
        let Some(daemon) = daemon else { return };
        let receiver = match daemon.browse(MDNS_TYPE) {
            Ok(receiver) => receiver,
            Err(err) => {
                self.note(format!("Could not browse the local network: {err}"));
                return;
            }
        };
        let (tx, mut rx) = mpsc::unbounded_channel();
        std::thread::spawn(move || {
            while let Ok(event) = receiver.recv() {
                if tx.send(event).is_err() {
                    break;
                }
            }
        });
        let engine = self.clone();
        tokio::spawn(async move {
            while let Some(event) = rx.recv().await {
                if let ServiceEvent::ServiceResolved(info) = event {
                    engine.consider_advert(&info).await;
                }
            }
        });
    }

    async fn consider_advert(&self, info: &ServiceInfo) {
        let device_id = info.get_property_val_str("deviceId").unwrap_or("");
        let fp = info.get_property_val_str("fp").unwrap_or("");
        let own_fp = self.lock().identity.fp_hex.clone();
        let own_nonce = protocol::b64(&self.lock().nonce);
        if fp.eq_ignore_ascii_case(&own_fp) || device_id == own_nonce {
            return;
        }
        let Some(prefix) = protocol::b64_decode(device_id) else { return };
        let phone = {
            let guard = self.lock();
            guard.phones.iter().find_map(|phone| {
                let real = protocol::b64_decode(&phone.real_id)?;
                protocol::nonce_matches(&prefix, &real).then(|| phone.clone())
            })
        };
        let Some(phone) = phone else { return };
        if phone.user_disconnected || self.lock().senders.contains_key(&phone.real_id) {
            return;
        }
        if self.lock().last_invite.get(&phone.real_id).is_some_and(|at| at.elapsed() < INVITE_GAP) {
            return;
        }
        let port = info.get_port();
        let Some(ip) = info.get_addresses().iter().find_map(|addr| match addr {
            IpAddr::V4(ip) if !ip.is_loopback() && !ip.is_link_local() => Some(*ip),
            _ => None,
        }) else {
            return;
        };
        if port == 0 {
            return;
        }
        self.lock().last_invite.insert(phone.real_id.clone(), Instant::now());
        self.note(format!("Calling {} at {ip}:{port}.", phone.name));
        if let Err(err) = self.push_invite(ip, port, &phone.our_key).await {
            self.note(format!("Call to {} failed: {err}", phone.name));
        }
    }

    async fn push_invite(&self, ip: Ipv4Addr, port: u16, key: &str) -> Result<()> {
        let Some(frame) = self.sealed_invite(key) else {
            anyhow::bail!("invite was not sealed");
        };
        let tcp = TcpStream::connect((ip, port)).await?;
        let config = self
            .shared
            .client_tls
            .lock()
            .unwrap_or_else(|err| err.into_inner())
            .clone()
            .context("tls client is not ready")?;
        let connector = TlsConnector::from(config);
        let name = ServerName::try_from("localhost").context("server name")?;
        let tls = connector.connect(name, tcp).await?;
        let url = format!("wss://{ip}:{port}/readyfor/mdns");
        let (mut socket, _) = client_async(url, tls).await?;
        socket.send(Message::Binary(frame.into())).await?;
        tokio::time::sleep(Duration::from_millis(300)).await;
        let _ = socket.close(None).await;
        Ok(())
    }

    fn rotate_token(&self) {
        let token = protocol::mint_token();
        let payload = {
            let mut guard = self.lock();
            if !guard.pairing {
                return;
            }
            let now = Instant::now();
            guard.tokens.retain(|token| token.expires > now);
            let expires = now + Duration::from_secs(TOKEN_TTL_SECS);
            guard.ips = net::local_ipv4s();
            let payload = protocol::qr_payload(
                &protocol::b64(&guard.nonce),
                &guard.identity.real_id,
                &guard.identity.device_name,
                &token,
                &guard.identity.fp_b64,
                guard.port.max(1),
                &guard.ips,
            );
            guard.tokens.push(LiveToken { value: token, expires, pairing: true });
            guard.qr_expires = Some(expires);
            payload
        };
        match protocol::qr_svg(&payload) {
            Ok(svg) => {
                let mut guard = self.lock();
                if guard.pairing {
                    guard.qr_svg = svg;
                }
            }
            Err(err) => self.note(format!("Could not draw the code: {err}")),
        }
    }

    fn token_wait(&self) -> Duration {
        let guard = self.lock();
        guard
            .qr_expires
            .map(|deadline| deadline.saturating_duration_since(Instant::now()))
            .filter(|wait| !wait.is_zero())
            .unwrap_or(Duration::from_secs(1))
    }

    fn token_matches(&self, token: &str) -> bool {
        if token.is_empty() {
            return false;
        }
        let now = Instant::now();
        self.lock().tokens.iter().any(|item| item.expires > now && protocol::tokens_equal(&item.value, token))
    }

    fn pair_key_for(&self, real_id: &str) -> String {
        let mut guard = self.lock();
        if let Some(phone) = guard.phones.iter().find(|phone| phone.real_id == real_id) {
            return phone.our_key.clone();
        }
        if let Some(key) = guard.pending_keys.get(real_id) {
            return key.clone();
        }
        let key = protocol::mint_pair_key();
        guard.pending_keys.insert(real_id.to_string(), key.clone());
        key
    }

    fn mark_user_disconnected(&self, real_id: &str) {
        let mut guard = self.lock();
        if let Some(phone) = guard.phones.iter_mut().find(|phone| phone.real_id == real_id) {
            phone.user_disconnected = true;
        }
        let identity = guard.identity.clone();
        let phones = guard.phones.clone();
        drop(guard);
        let _ = store::save(&identity, &phones);
    }

    fn lock_screen(&self) {
        let engine = self.clone();
        tokio::task::spawn_blocking(move || {
            let result = std::process::Command::new("osascript")
                .arg("-e")
                .arg("tell application \"System Events\" to key code 12 using {control down, command down}")
                .output();
            match result {
                Ok(output) if output.status.success() => engine.note("Screen locked."),
                Ok(output) => {
                    let detail = String::from_utf8_lossy(&output.stderr);
                    engine.note(format!(
                        "Could not lock the screen. Allow this app to control System Events in Privacy & Security. {detail}"
                    ));
                }
                Err(err) => engine.note(format!("Could not lock the screen: {err}")),
            }
            engine.publish();
        });
    }

    fn on_sms(&self, service: i64, cmd: i64, data: &Value) {
        let outcome = self.lock().sms.on_message(service, cmd, data);
        self.flush_sms(outcome);
    }

    fn flush_sms(&self, outcome: sms::Outcome) {
        for data in outcome.sends {
            self.send_active(204, data);
        }
        for line in outcome.notes {
            self.note(line);
        }
        if outcome.changed {
            self.publish();
        }
    }

    pub fn messages_open(&self, key: &str) -> Result<(), String> {
        let outcome = self.lock().sms.open(key)?;
        self.flush_sms(outcome);
        Ok(())
    }

    pub fn messages_close(&self) {
        let outcome = self.lock().sms.close();
        self.flush_sms(outcome);
    }

    pub fn messages_send(&self, text: &str) -> Result<(), String> {
        self.require_phone()?;
        let outcome = self.lock().sms.send_open(text)?;
        self.flush_sms(outcome);
        Ok(())
    }

    pub fn messages_send_new(&self, address: &str, text: &str) -> Result<(), String> {
        self.require_phone()?;
        let outcome = self.lock().sms.send_new(address, text)?;
        self.flush_sms(outcome);
        Ok(())
    }

    pub fn notification_reply(&self, key: &str, text: &str) -> Result<(), String> {
        self.require_phone()?;
        let effects = self.lock().desk.reply(key, text)?;
        self.apply_effects(effects, None);
        Ok(())
    }

    pub fn notification_act(&self, key: &str, action_id: i64) -> Result<(), String> {
        self.require_phone()?;
        let effects = self.lock().desk.act(key, action_id)?;
        self.apply_effects(effects, None);
        Ok(())
    }

    pub fn notification_open(&self, key: &str) -> Result<(), String> {
        self.require_phone()?;
        let effects = self.lock().desk.open_notice(key)?;
        self.apply_effects(effects, None);
        Ok(())
    }

    pub fn start_desktop(&self) -> Result<(), String> {
        self.require_phone()?;
        let effects = self.lock().desk.start_desktop()?;
        self.apply_effects(effects, None);
        Ok(())
    }

    pub async fn list_phone(&self, path: &str) -> Result<Vec<nfs::PhoneEntry>, String> {
        let parts = nfs::clean_path(path).map_err(|err| err.to_string())?;
        self.ensure_share().await?;
        let (ip, share, cert, key) = self.share_parts()?;
        match nfs::list_dir(ip, &share, &cert, &key, &parts).await {
            Ok(entries) => Ok(entries),
            Err(err) => {
                self.lock().desk.clear_share();
                Err(err.to_string())
            }
        }
    }

    pub async fn save_phone(&self, path: &str) -> Result<String, String> {
        let parts = nfs::clean_path(path).map_err(|err| err.to_string())?;
        if parts.is_empty() {
            return Err("Choose a file.".into());
        }
        self.ensure_share().await?;
        let (ip, share, cert, key) = self.share_parts()?;
        let bytes = match nfs::read_file(ip, &share, &cert, &key, &parts).await {
            Ok(bytes) => bytes,
            Err(err) => {
                self.lock().desk.clear_share();
                return Err(err.to_string());
            }
        };
        let name = parts.last().cloned().unwrap_or_default();
        let saved = write_phone_file(&name, &bytes).map_err(|err| err.to_string())?;
        self.lock().desk.store_files(vec![FileView { name, path: saved.clone() }]);
        self.note("Saved a file from the phone.");
        self.publish();
        Ok(saved)
    }

    async fn ensure_share(&self) -> Result<(), String> {
        if self.lock().senders.is_empty() {
            return Err("Connect the phone first.".into());
        }
        if self.lock().desk.phone_share().is_some() {
            return Ok(());
        }
        let generation = self.lock().desk.share_generation();
        let fingerprint = self.lock().identity.fp_hex.clone();
        self.send_active(168, Desk::phone_browse_request(&fingerprint));
        let deadline = Instant::now() + Duration::from_secs(8);
        loop {
            let left = deadline.saturating_duration_since(Instant::now());
            if left.is_zero() {
                return Err("The phone did not open its folders. Unlock it and keep Smart Connect in front.".into());
            }
            if tokio::time::timeout(left, self.shared.share_ready.notified()).await.is_err() {
                return Err("The phone did not open its folders. Unlock it and keep Smart Connect in front.".into());
            }
            let guard = self.lock();
            if guard.desk.share_generation() > generation && guard.desk.phone_share().is_some() {
                return Ok(());
            }
        }
    }

    fn share_parts(&self) -> Result<(IpAddr, nfs::PhoneShare, String, String), String> {
        let guard = self.lock();
        let ip = guard
            .senders
            .iter()
            .max_by_key(|(_, (session, _))| *session)
            .and_then(|(id, _)| guard.peers.get(id).copied())
            .ok_or_else(|| "Connect the phone first.".to_string())?;
        let share = guard.desk.phone_share().ok_or_else(|| "The phone did not open its folders.".to_string())?;
        Ok((ip, share, guard.identity.cert_pem.clone(), guard.identity.key_pem.clone()))
    }

    pub fn start_mirror(&self) -> Result<(), String> {
        self.require_phone()?;
        let effects = self.lock().desk.start_mirror()?;
        self.apply_effects(effects, None);
        Ok(())
    }

    pub fn start_app(&self, pkg: &str, cmp: &str, user_id: i64) -> Result<(), String> {
        self.require_phone()?;
        let effects = self.lock().desk.start_app(pkg, cmp, user_id)?;
        self.apply_effects(effects, None);
        Ok(())
    }

    pub fn start_webcam(&self) -> Result<(), String> {
        self.require_phone()?;
        crate::cam_publish::ensure();
        let effects = self.lock().desk.start_webcam()?;
        self.apply_effects(effects, None);
        Ok(())
    }

    pub fn webcam_flip(&self) -> Result<(), String> {
        self.require_phone()?;
        let effects = self.lock().desk.webcam_flip()?;
        self.apply_effects(effects, None);
        Ok(())
    }

    pub fn play_decoded(&self, session_id: &str, sdp: &str) -> Result<(), String> {
        if session_id.is_empty() || sdp.len() > 100_000 || !sdp.contains("v=") {
            return Err("The phone's screen could not be played.".into());
        }
        let sdp = crate::desk::h264_only_sdp(sdp);
        let (lan, phone) = self.media_addresses();
        let previous = self.shared.mirror.begin(session_id);
        if let Some(previous) = previous {
            tauri::async_runtime::spawn(async move {
                let _ = previous.close().await;
            });
        }
        let engine = self.clone();
        let mirror = Arc::clone(&self.shared.mirror);
        let session_id = session_id.to_string();
        tauri::async_runtime::spawn(async move {
            mirror.answer(engine, session_id, sdp, lan, phone).await;
        });
        self.note("The window could not play the phone directly. Decoding it here.");
        Ok(())
    }

    pub fn stop_stream(&self, session_id: &str) {
        let effects = self.lock().desk.stop_session(session_id);
        if effects.is_empty() {
            self.note("There is no screen to close.");
            return;
        }
        self.apply_effects(effects, None);
    }

    pub fn stop_pkg(&self, pkg: &str) {
        let effects = self.lock().desk.stop_pkg(pkg);
        if effects.is_empty() {
            return;
        }
        self.apply_effects(effects, None);
    }

    pub fn webrtc_answer(&self, session_id: &str, sdp: &str) -> Result<(), String> {
        self.forget_offer(session_id);
        let effects = self.lock().desk.answer(session_id, sdp)?;
        self.apply_effects(effects, None);
        Ok(())
    }

    pub fn mirror_view(&self, since: u64) -> crate::mirror::MirrorView {
        self.shared.mirror.view(since)
    }

    pub fn show_frame(&self, seq: u64, jpeg: &str, width: u32, height: u32, session_id: &str) {
        if jpeg.is_empty() {
            return;
        }
        let _ = self.shared.app.emit(
            "frame",
            json!({ "seq": seq, "jpeg": jpeg, "width": width, "height": height, "sessionId": session_id }),
        );
    }

    pub fn screen_note(&self, text: &str) {
        let trimmed = text.trim();
        if trimmed.is_empty() || trimmed.len() > 200 {
            return;
        }
        let lower = trimmed.to_ascii_lowercase();
        if lower.contains("v=0") || lower.contains("a=candidate") || lower.contains("fingerprint") {
            return;
        }
        self.note(trimmed.to_string());
        self.publish();
    }

    pub fn webrtc_ice(&self, session_id: &str, candidate: &str, label: i64, mid: &str) {
        let Some(effect) = self.lock().desk.ice(session_id, candidate, label, mid) else { return };
        self.apply_effects(vec![effect], None);
    }

    pub fn cross_start(&self) -> Result<(), String> {
        self.require_phone()?;
        let effects = self.lock().desk.cross_start();
        self.apply_effects(effects, None);
        Ok(())
    }

    pub fn cross_stop(&self) {
        let effects = self.lock().desk.cross_stop();
        self.apply_effects(effects, None);
    }

    pub fn cross_show(&self, show: bool) {
        let Some(effect) = self.lock().desk.cross_show(show) else { return };
        self.apply_effects(vec![effect], None);
    }

    pub fn send_input(&self, display_id: i64, kind: &str, x: i64, y: i64, key: i64, down: bool, flags: i64, text: &str) {
        if self.lock().senders.is_empty() {
            return;
        }
        let x = x.clamp(0, 20_000);
        let y = y.clamp(0, 20_000);
        let data = match kind {
            "move" => json!({ "cmd": 4, "data": { "displayId": display_id, "x": x, "y": y } }),
            "button" | "wheel" => json!({ "cmd": 5, "data": { "displayId": display_id, "key": key, "x": x, "y": y, "down": down } }),
            "key" => json!({ "cmd": 3, "data": { "displayId": display_id, "flags": flags, "key": key, "down": down, "metaState": 0 } }),
            // Service 16 cmd 9. event is 1 down, 2 move, 0 up. The phone only
            // delivers this on a screen that reported supportTouch.
            "touch" => {
                let event = key.clamp(0, 3);
                json!({
                    "cmd": 9,
                    "data": {
                        "displayId": display_id,
                        "event": event,
                        "pointerId": 0,
                        "x": x,
                        "y": y,
                        "pressure": if event == 0 { 0.0 } else { 1.0 },
                        "majorAxisSize": 12.0
                    }
                })
            }
            "text" => {
                let text: String = text.chars().take(4000).collect();
                if text.is_empty() {
                    return;
                }
                json!({ "cmd": 7, "data": { "displayId": display_id, "text": text } })
            }
            _ => return,
        };
        let effects = self.lock().desk.input(display_id, data);
        self.apply_effects(effects, None);
    }

    pub fn refresh_apps(&self) -> Result<(), String> {
        self.require_phone()?;
        let effects = self.lock().desk.refresh_apps();
        self.apply_effects(effects, None);
        Ok(())
    }

    pub fn app_icons(&self) -> IconPack {
        let guard = self.lock();
        IconPack { revision: guard.desk.icons_revision, icons: guard.desk.icons() }
    }

    pub fn reveal_file(&self, path: &str) -> Result<(), String> {
        if !self.lock().desk.known_file(path) {
            return Err("That file is not from the phone.".into());
        }
        let root = receive_root();
        let canonical = PathBuf::from(path).canonicalize().map_err(|_| "That file is no longer there.".to_string())?;
        let root = root.canonicalize().map_err(|_| "The downloads folder is not available.".to_string())?;
        if !canonical.starts_with(&root) {
            return Err("That file is not from the phone.".into());
        }
        let status = std::process::Command::new("open").arg(&canonical).status().map_err(|err| err.to_string())?;
        if status.success() { Ok(()) } else { Err("Could not open the file.".into()) }
    }

    fn require_phone(&self) -> Result<(), String> {
        if self.lock().senders.is_empty() {
            Err("Connect the phone first.".into())
        } else {
            Ok(())
        }
    }

    fn heartbeat(&self) {
        if self.lock().desk.has_sessions() {
            self.send_active(15, json!({ "ver": "1", "cmd": 9 }));
        }
        if self.lock().desk.webcam_heartbeat() {
            self.send_active(3, json!({ "ver": "1", "cmd": 13 }));
        }
        self.refresh_power();
    }

    /// The phone keeps the last battery it was sent. Push a new one when the level or the plug changes.
    fn refresh_power(&self) {
        if self.lock().senders.is_empty() {
            return;
        }
        let power = crate::power::read();
        let previous = self.lock().reported_power;
        if previous == Some(power) {
            return;
        }
        let changed_state = previous.is_some_and(|previous| previous.charging != power.charging || previous.plugged != power.plugged);
        self.send_status();
        if changed_state {
            self.note(power.line());
        }
    }

    fn poll_clipboard(&self) {
        if !self.lock().desk.clipboard_ready {
            return;
        }
        let engine = self.clone();
        tokio::task::spawn_blocking(move || {
            let Ok(output) = std::process::Command::new("pbpaste").output() else { return };
            if !output.status.success() {
                return;
            }
            let Ok(text) = String::from_utf8(output.stdout) else { return };
            if text.is_empty() {
                return;
            }
            let data = engine.lock().desk.clipboard_out(&text);
            if let Some(data) = data {
                engine.send_active(9, data);
                engine.note("Sent a copy to the phone.");
                engine.publish();
            }
        });
    }

    fn apply_effects(&self, effects: Vec<Effect>, tx: Option<&mpsc::UnboundedSender<Message>>) {
        let mut publish = false;
        for effect in effects {
            match effect {
                Effect::Send { service, data } => {
                    if let Some(tx) = tx {
                        self.send_json(tx, service, data);
                    } else {
                        self.send_active(service, data);
                    }
                }
                Effect::Note(line) => self.note(line),
                Effect::Publish => publish = true,
                Effect::Banner { title, body } => self.show_banner(title, body),
                Effect::Paste(text) => self.write_clipboard(text),
                Effect::Pull(job) => self.spawn_pull(job),
                Effect::Offer { session_id, sdp, quiet, pkg } => {
                    // The camera, an app, and the phone screen are answered in a window
                    // from the offer the phone sent. Decoding one here replaces the other.
                    self.remember_offer(&session_id, &sdp, quiet, &pkg);
                    self.emit_rtc("offer", &session_id, &sdp, "", 0, "", quiet, &pkg);
                }
                Effect::Ice { session_id, candidate, label, mid } => {
                    let (_, phone) = self.media_addresses();
                    let candidate = rewrite_mdns(&candidate, &phone);
                    if self.shared.mirror.owns(&session_id) {
                        let mirror = Arc::clone(&self.shared.mirror);
                        tauri::async_runtime::spawn(async move {
                            mirror.add_remote(&session_id, &candidate, label, &mid).await;
                        });
                    } else {
                        let pkg = self.lock().desk.session_pkg(&session_id);
                        self.emit_rtc("ice", &session_id, "", &candidate, label, &mid, false, &pkg);
                    }
                }
                Effect::Obs { on } => crate::cam_publish::set_open(on),
                Effect::Closed { session_id } => {
                    let mirror = Arc::clone(&self.shared.mirror);
                    let id = session_id.clone();
                    tauri::async_runtime::spawn(async move {
                        mirror.close(&id).await;
                    });
                    self.forget_offer(&session_id);
                    self.emit_rtc("closed", &session_id, "", "", 0, "", false, "");
                }
                Effect::WatchStart { token } => self.watch_start(token),
                Effect::ShareReady => self.shared.share_ready.notify_one(),
                Effect::CloseLater { session_id, secs } => {
                    let engine = self.clone();
                    tokio::spawn(async move {
                        tokio::time::sleep(Duration::from_secs(secs)).await;
                        let effects = engine.lock().desk.stop_session(&session_id);
                        engine.apply_effects(effects, None);
                    });
                }
                Effect::Status => self.send_status(),
            }
        }
        if publish {
            self.publish();
        }
    }

    fn send_status(&self) {
        let power = crate::power::read();
        let text = {
            let mut guard = self.lock();
            guard.reported_power = Some(power);
            protocol::status_message_for(
                &protocol::b64(&guard.nonce),
                &guard.identity.device_name,
                self.msg_id(),
                guard.desk.cross_active,
                power,
            )
        };
        if let Some(tx) = self.active_sender() {
            let _ = tx.send(Message::Text(text.into()));
        }
    }

    fn send_active(&self, service: i64, data: Value) {
        if let Some(tx) = self.active_sender() {
            self.send_json(&tx, service, data);
        }
    }

    fn send_json(&self, tx: &mpsc::UnboundedSender<Message>, service: i64, data: Value) {
        let text = {
            let guard = self.lock();
            protocol::control_message(&protocol::b64(&guard.nonce), service, self.msg_id(), &data)
        };
        let _ = tx.send(Message::Text(text.into()));
    }

    fn active_sender(&self) -> Option<mpsc::UnboundedSender<Message>> {
        let guard = self.lock();
        guard.senders.values().max_by_key(|(id, _)| *id).map(|(_, tx)| tx.clone())
    }

    fn active_ip(&self) -> Option<IpAddr> {
        let guard = self.lock();
        let id = guard.senders.iter().max_by_key(|(_, (session, _))| *session).map(|(id, _)| id.clone());
        id.and_then(|id| guard.peers.get(&id).copied())
    }

    fn watch_start(&self, token: u64) {
        let engine = self.clone();
        // Commands run on the window thread, which is outside the socket runtime.
        tauri::async_runtime::spawn(async move {
            tokio::time::sleep(Duration::from_secs(20)).await;
            if engine.lock().desk.give_up(token) {
                engine.note("The phone did not open a screen. Unlock it, keep Smart Connect in front, and try again.");
                engine.publish();
            }
        });
    }

    fn media_addresses(&self) -> (String, String) {
        let guard = self.lock();
        let session = guard.senders.values().map(|(id, _)| *id).max();
        let mac = session
            .and_then(|id| guard.session_locals.get(&id))
            .map(ToString::to_string)
            .or_else(|| guard.ips.iter().find(|ip| *ip != "127.0.0.1").cloned())
            .unwrap_or_default();
        let phone = session
            .and_then(|id| guard.session_peers.get(&id))
            .map(ToString::to_string)
            .filter(|ip| !ip.starts_with("127."))
            .unwrap_or_default();
        (mac, phone)
    }

    fn remember_offer(&self, session_id: &str, sdp: &str, quiet: bool, pkg: &str) {
        let mut offers = self.shared.page_offers.lock().unwrap_or_else(|err| err.into_inner());
        offers.retain(|offer| offer.session_id != session_id);
        offers.push(PageOffer {
            session_id: session_id.to_string(),
            sdp: sdp.to_string(),
            quiet,
            pkg: pkg.to_string(),
        });
    }

    fn forget_offer(&self, session_id: &str) {
        let mut offers = self.shared.page_offers.lock().unwrap_or_else(|err| err.into_inner());
        offers.retain(|offer| offer.session_id != session_id);
    }

    pub fn page_offers(&self) -> Vec<Value> {
        let offers = self.shared.page_offers.lock().unwrap_or_else(|err| err.into_inner());
        let (lan_ip, phone_ip) = self.media_addresses();
        offers
            .iter()
            .map(|offer| {
                json!({
                    "kind": "offer",
                    "sessionId": offer.session_id,
                    "sdp": offer.sdp,
                    "candidate": "",
                    "label": 0,
                    "mid": "",
                    "quiet": offer.quiet,
                    "pkg": offer.pkg,
                    "lanIp": lan_ip,
                    "phoneIp": phone_ip,
                })
            })
            .collect()
    }

    fn emit_rtc(&self, kind: &str, session_id: &str, sdp: &str, candidate: &str, label: i64, mid: &str, quiet: bool, pkg: &str) {
        let (lan_ip, phone_ip) = self.media_addresses();
        let _ = self.shared.app.emit(
            "rtc",
            json!({
                "kind": kind,
                "sessionId": session_id,
                "sdp": sdp,
                "candidate": candidate,
                "label": label,
                "mid": mid,
                "quiet": quiet,
                "pkg": pkg,
                "lanIp": lan_ip,
                "phoneIp": phone_ip
            }),
        );
    }

    fn show_banner(&self, title: String, body: String) {
        let engine = self.clone();
        tokio::task::spawn_blocking(move || {
            let body = if body.is_empty() { " ".to_string() } else { body };
            let result = std::process::Command::new("osascript")
                .arg("-e")
                .arg("on run argv\ndisplay notification item 2 of argv with title item 1 of argv\nend run")
                .arg("--")
                .arg(&title)
                .arg(&body)
                .status();
            if !matches!(result, Ok(status) if status.success()) && engine.lock().desk.mark_banner_failed() {
                engine.note("macOS did not show a banner. Allow notifications for this app to see them outside the window.");
                engine.publish();
            }
        });
    }

    fn write_clipboard(&self, text: String) {
        let engine = self.clone();
        tokio::task::spawn_blocking(move || {
            let mut child = match std::process::Command::new("pbcopy").stdin(std::process::Stdio::piped()).spawn() {
                Ok(child) => child,
                Err(err) => {
                    engine.note(format!("Could not write the Mac clipboard ({err})."));
                    return;
                }
            };
            if let Some(mut stdin) = child.stdin.take() {
                use std::io::Write;
                let _ = stdin.write_all(text.as_bytes());
            }
            let _ = child.wait();
        });
    }

    fn spawn_pull(&self, job: desk::PullJob) {
        let Some(ip) = self.active_ip() else {
            self.note("The phone offered a file, but its address is not known yet.");
            return;
        };
        let engine = self.clone();
        tokio::spawn(async move {
            let stamp = unix_millis();
            let zip_path = std::env::temp_dir().join(format!("smartconnect-{stamp}.zip"));
            match fex::pull_zip(ip, job.port, &job.fingerprint, &job.token, &zip_path).await {
                Ok(_) if job.icons => {
                    let icons = unpack_icons(&zip_path);
                    let _ = std::fs::remove_file(&zip_path);
                    let count = icons.len();
                    engine.lock().desk.store_icons(icons);
                    if count > 0 {
                        engine.note(format!("Saved {count} app icons."));
                    }
                    engine.publish();
                }
                Ok(_) => {
                    let dir = receive_root().join(stamp.to_string());
                    let saved = unpack_files(&zip_path, &dir);
                    let _ = std::fs::remove_file(&zip_path);
                    if saved.is_empty() {
                        engine.note("The phone's file arrived empty.");
                    } else {
                        let count = saved.len();
                        engine.lock().desk.store_files(saved);
                        if count == 1 {
                            engine.note("Saved 1 file from the phone.");
                        } else {
                            engine.note(format!("Saved {count} files from the phone."));
                        }
                    }
                    engine.publish();
                }
                Err(err) => {
                    let _ = std::fs::remove_file(&zip_path);
                    let what = if job.icons { "app icons" } else { "a file from the phone" };
                    engine.note(format!("Could not save {what} ({err:#})."));
                    engine.publish();
                }
            }
        });
    }

    fn msg_id(&self) -> u64 {
        self.shared.next_msg.fetch_add(1, Ordering::Relaxed)
    }

    fn lock(&self) -> MutexGuard<'_, Inner> {
        self.shared.inner.lock().unwrap_or_else(|err| err.into_inner())
    }

    fn mdns_mut(&self) -> MutexGuard<'_, Option<ServiceDaemon>> {
        self.shared.mdns.lock().unwrap_or_else(|err| err.into_inner())
    }

    fn note(&self, line: impl AsRef<str>) {
        let line = format!("{}  {}", stamp(), line.as_ref());
        self.lock().logs.push_back(line);
        while self.lock().logs.len() > 80 {
            self.lock().logs.pop_front();
        }
    }

    fn fail(&self, message: String) {
        self.lock().error = Some(message.clone());
        self.note(message);
        self.publish();
    }

    fn publish(&self) {
        let _ = self.shared.app.emit("snapshot", self.snapshot());
    }
}

fn non_empty_str(value: Option<&Value>) -> Option<String> {
    value.and_then(Value::as_str).map(str::trim).filter(|text| !text.is_empty()).map(str::to_string)
}

fn mdns_instance(name: &str) -> String {
    let mut out = String::new();
    for ch in name.chars() {
        if out.len() + ch.len_utf8() > 60 {
            break;
        }
        out.push(ch);
    }
    if out.is_empty() { "Mac".to_string() } else { out }
}

fn stamp() -> String {
    let secs = SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0);
    format!("{:02}:{:02}:{:02}Z", (secs / 3600) % 24, (secs / 60) % 60, secs % 60)
}

fn unix_secs() -> u64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0)
}

fn unix_millis() -> u128 {
    SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_millis()).unwrap_or(0)
}

fn find_wallpaper(value: &Value) -> Option<(String, String)> {
    match value {
        Value::Object(map) => {
            if let Some(raw) = map.get("wallpaper").and_then(Value::as_str) {
                if raw.len() >= 32 {
                    let md5 = map.get("md5").and_then(Value::as_str).unwrap_or("").to_string();
                    return Some((raw.to_string(), md5));
                }
            }
            map.values().find_map(find_wallpaper)
        }
        Value::Array(items) => items.iter().find_map(find_wallpaper),
        Value::String(text) if text.contains("wallpaper") => {
            let parsed = serde_json::from_str::<Value>(&protocol::quote_wide_integers(text)).ok()?;
            find_wallpaper(&parsed)
        }
        _ => None,
    }
}

fn value_keys(value: &Value) -> String {
    let Some(map) = value.as_object() else {
        return "none".into();
    };
    let mut keys: Vec<&str> = map.keys().map(String::as_str).collect();
    keys.sort_unstable();
    if keys.is_empty() { "none".into() } else { keys.join(", ") }
}

fn wallpaper_data_url(raw: &str) -> Option<String> {
    let compact: String = raw.chars().filter(|ch| !ch.is_whitespace()).collect();
    if compact.len() < 32 || compact.len() > 6_000_000 {
        return None;
    }
    let bytes = protocol::b64_decode(&compact)?;
    if bytes.len() < 32 || bytes.len() > 4_000_000 {
        return None;
    }
    let mime = if bytes.starts_with(&[0xFF, 0xD8, 0xFF]) {
        "image/jpeg"
    } else if bytes.starts_with(b"\x89PNG\r\n\x1a\n") {
        "image/png"
    } else if bytes.len() > 12 && &bytes[..4] == b"RIFF" && &bytes[8..12] == b"WEBP" {
        "image/webp"
    } else {
        return None;
    };
    Some(format!("data:{mime};base64,{compact}"))
}

fn is_feature(service: i64) -> bool {
    matches!(service, 1 | 2 | 3 | 5 | 9 | 15 | 16 | 17 | 26 | 37 | 168)
}

fn write_phone_file(name: &str, bytes: &[u8]) -> Result<String> {
    let clean = std::path::Path::new(name)
        .file_name()
        .and_then(|name| name.to_str())
        .filter(|name| !name.is_empty() && !name.contains('/') && !name.contains('\\') && *name != "." && *name != "..")
        .ok_or_else(|| anyhow::anyhow!("That file name cannot be saved."))?;
    let dir = receive_root();
    std::fs::create_dir_all(&dir).map_err(|_| anyhow::anyhow!("The downloads folder is not available."))?;
    let mut path = dir.join(clean);
    if path.exists() {
        let stem = path.file_stem().and_then(|stem| stem.to_str()).unwrap_or("file");
        let ext = path.extension().and_then(|ext| ext.to_str()).unwrap_or("");
        let suffix = unix_millis();
        let named = if ext.is_empty() {
            format!("{stem}-{suffix}")
        } else {
            format!("{stem}-{suffix}.{ext}")
        };
        path = dir.join(named);
    }
    std::fs::write(&path, bytes).map_err(|_| anyhow::anyhow!("The file could not be saved."))?;
    Ok(path.to_string_lossy().to_string())
}

fn receive_root() -> PathBuf {
    dirs::download_dir().unwrap_or_else(std::env::temp_dir).join("Smart Connect")
}

fn unpack_files(zip: &std::path::Path, dir: &std::path::Path) -> Vec<FileView> {
    if std::fs::create_dir_all(dir).is_err() || !unzip_flat(zip, dir) {
        return Vec::new();
    }
    let mut files = Vec::new();
    let Ok(entries) = std::fs::read_dir(dir) else { return files };
    for entry in entries.flatten() {
        let name = entry.file_name().to_string_lossy().to_string();
        if name.starts_with('.') || name.contains('/') || name.contains('\\') {
            continue;
        }
        let Ok(meta) = entry.metadata() else { continue };
        if !meta.is_file() {
            continue;
        }
        files.push(FileView { name, path: entry.path().to_string_lossy().to_string() });
    }
    files
}

fn unpack_icons(zip: &std::path::Path) -> HashMap<String, String> {
    let dir = std::env::temp_dir().join(format!("smartconnect-icons-{}", unix_millis()));
    let mut icons = HashMap::new();
    if std::fs::create_dir_all(&dir).is_err() || !unzip_flat(zip, &dir) {
        let _ = std::fs::remove_dir_all(&dir);
        return icons;
    }
    if let Ok(entries) = std::fs::read_dir(&dir) {
        for entry in entries.flatten() {
            let name = entry.file_name().to_string_lossy().to_string();
            let Ok(bytes) = std::fs::read(entry.path()) else { continue };
            if let Some((name, url)) = desk::decode_icon(&name, &bytes) {
                icons.insert(name, url);
            }
        }
    }
    let _ = std::fs::remove_dir_all(&dir);
    icons
}

fn media_ipv4(addr: SocketAddr) -> Option<IpAddr> {
    match addr.ip() {
        IpAddr::V4(ip) if !ip.is_loopback() && !ip.is_link_local() && !ip.is_unspecified() && !ip.is_multicast() => Some(IpAddr::V4(ip)),
        _ => None,
    }
}

/// Apple hides host addresses as `*.local`. The phone can only dial the real LAN address.
fn rewrite_mdns(candidate: &str, ip: &str) -> String {
    if ip.is_empty() || !candidate.contains(".local") {
        return candidate.to_string();
    }
    candidate
        .split(' ')
        .map(|part| if part.ends_with(".local") { ip } else { part })
        .collect::<Vec<_>>()
        .join(" ")
}

fn unzip_flat(zip: &std::path::Path, dir: &std::path::Path) -> bool {
    std::process::Command::new("/usr/bin/unzip")
        .args(["-qq", "-j", "-o"])
        .arg(zip)
        .arg("-d")
        .arg(dir)
        .status()
        .map(|status| status.success())
        .unwrap_or(false)
}

#[cfg(test)]
mod wallpaper_tests {
    use super::{find_wallpaper, wallpaper_data_url};
    use base64::Engine;
    use serde_json::json;

    #[test]
    fn a_stringified_status_still_carries_the_picture() {
        let mut bytes = vec![0xFF, 0xD8, 0xFF];
        bytes.extend(std::iter::repeat(0u8).take(40));
        let picture = base64::engine::general_purpose::STANDARD.encode(&bytes);
        let inner = json!({ "cmd": 2, "md5": "abc", "wallpaper": picture }).to_string();
        let message = json!({ "serviceId": 25, "data": inner });
        let found = find_wallpaper(&message).expect("picture");
        assert_eq!(found.1, "abc");
        let url = wallpaper_data_url(&found.0).expect("jpeg");
        assert!(url.starts_with("data:image/jpeg;base64,"));
    }
}
