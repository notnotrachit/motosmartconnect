//! Lenovo ID / Motorola account sign-in, then the cloud broker device list.
//!
//! The browser step matches the Windows client: a public PKCE authorize URL and
//! the `smartconnect:authcode` return. Tokens stay in Application Support and
//! are never written to the log. Wi-Fi and Bluetooth wake do not depend on this.

use crate::cloud;
use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use base64::Engine;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::os::unix::fs::OpenOptionsExt;
use std::path::PathBuf;
use std::sync::Mutex;
use std::time::{SystemTime, UNIX_EPOCH};
use tauri::{AppHandle, Emitter};

const CLIENT_ID: &str = "4d7271f56c7d5c0521e855395b20911265aaf256957dd3da11b288fc939ebd80";
const REDIRECT: &str = "smartconnect:authcode";
const AUTHORIZE: &str = "https://passport.lenovo.com/v1.0/utility/lenovoid/oauth2/authorize";
const PASSPORT_TOKEN: &str = "https://passport.lenovo.com/v1.0/utility/lenovoid/oauth2/token";
const ST_URL: &str = "https://passport.lenovo.com/interserver/2.0/st/getbyaccesstoken";
const ACCOUNT_INFO: &str = "https://passport.lenovo.com/interserver/authen/1.2/getaccountinfo";
const MESH_CLIENT: &str = "smartconnect-pc";
const KEYCLOAKS: &[&str] = &[
    "https://account.naea1.uds.lenovo.com",
    "https://account.uds.lenovo.com",
];

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AccountDevice {
    pub id: String,
    pub name: String,
    pub online: bool,
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AccountView {
    pub signed_in: bool,
    pub name: String,
    pub detail: String,
    pub devices: Vec<AccountDevice>,
}

#[derive(Default, Clone, Serialize, Deserialize)]
struct Pending {
    state: String,
    verifier: String,
}

#[derive(Default, Clone, Serialize, Deserialize)]
struct Saved {
    #[serde(default)]
    access_token: String,
    #[serde(default)]
    refresh_token: String,
    #[serde(default)]
    access_expires: u64,
    #[serde(default)]
    lpsust: String,
    #[serde(default)]
    mesh_token: String,
    #[serde(default)]
    mesh_expires: u64,
    #[serde(default)]
    name: String,
    #[serde(default)]
    base_url: String,
    #[serde(default)]
    signaling_url: String,
    /// Lenovo ID realm from a passport callback, when the browser returned one.
    #[serde(default)]
    realm: String,
}

struct State {
    app: Option<AppHandle>,
    saved: Saved,
    pending: Option<Pending>,
    devices: Vec<AccountDevice>,
    detail: String,
    /// A browser callback is already being exchanged. A second one must not wipe that.
    busy: bool,
}

static STATE: Mutex<Option<State>> = Mutex::new(None);

pub fn install(app: AppHandle) {
    let saved = read_saved();
    let signed = !saved.access_token.is_empty();
    let detail = if signed {
        "Signed in. Refresh the account device list when you want to connect over the internet."
            .to_string()
    } else {
        idle_detail()
    };
    *lock() = Some(State {
        app: Some(app),
        saved,
        pending: None,
        devices: Vec::new(),
        detail,
        busy: false,
    });
}

pub fn accept_url(url: &str) {
    let handle = {
        let guard = lock();
        guard.as_ref().and_then(|state| state.app.clone())
    };
    let url = url.to_string();
    tauri::async_runtime::spawn(async move {
        let view = match finish(&url).await {
            Ok(view) => view,
            Err(err) => failed_view(&err),
        };
        if let Some(app) = handle {
            let _ = app.emit("account", &view);
        }
    });
}

pub fn view() -> AccountView {
    let guard = lock();
    match guard.as_ref() {
        Some(state) => state.view(),
        None => AccountView {
            signed_in: false,
            name: String::new(),
            detail: idle_detail(),
            devices: Vec::new(),
        },
    }
}

pub async fn begin() -> Result<AccountView, String> {
    let verifier = random_url(32);
    let challenge = pkce_challenge(&verifier);
    let state = random_url(16);
    let url = authorize_url(&challenge, &state);
    {
        let mut guard = lock();
        let slot = guard.as_mut().ok_or_else(|| "Account is not ready.".to_string())?;
        let pending = Pending { state: state.clone(), verifier: verifier.clone() };
        slot.pending = Some(pending.clone());
        slot.detail = "Waiting for Lenovo or Motorola sign-in in the browser.".to_string();
        write_secret(&pending_path()?, &pending)?;
    }
    let opened = std::process::Command::new("open")
        .arg(&url)
        .status()
        .map_err(|_| "Could not open the browser.".to_string())?
        .success();
    if !opened {
        set_detail("Could not open the browser.");
        return Err("Could not open the browser.".to_string());
    }
    Ok(view())
}

pub async fn finish(url: &str) -> Result<AccountView, String> {
    if let Some(session) = wust_from(url) {
        return finish_wust(session).await;
    }
    let (code, state) = callback_parts(url).ok_or_else(|| {
        "The sign-in redirect did not include a code. Paste the passport.lenovo.com address, or start sign-in again.".to_string()
    })?;
    let Some(pending) = take_pending(&state)? else {
        return Ok(view());
    };
    let exchanged = passport_exchange(&code, &pending.verifier).await;
    let tokens = match exchanged {
        Ok(tokens) => tokens,
        Err(err) => {
            clear_busy();
            return Err(err);
        }
    };
    if let Err(err) = store_passport(tokens).await {
        clear_busy();
        return Err(err);
    }
    if let Err(err) = refresh_devices_inner().await {
        set_detail(&format!("Signed in. The account device list is not available yet: {err}"));
    }
    clear_busy();
    Ok(view())
}

pub fn sign_out() -> AccountView {
    cloud::stop();
    if let Ok(path) = token_path() {
        let _ = fs::remove_file(path);
    }
    if let Ok(path) = pending_path() {
        let _ = fs::remove_file(path);
    }
    let mut guard = lock();
    if let Some(state) = guard.as_mut() {
        state.saved = Saved::default();
        state.pending = None;
        state.devices.clear();
        state.detail = idle_detail();
        state.busy = false;
    }
    drop(guard);
    view()
}

pub async fn refresh_devices() -> Result<AccountView, String> {
    refresh_devices_inner().await?;
    Ok(view())
}

pub async fn connect(name: &str, real_id: &str, port: u16, device_id: &str) -> Result<AccountView, String> {
    if port == 0 {
        return Err("This Mac is not listening yet.".to_string());
    }
    if let Err(err) = ensure_mesh().await {
        let message = if err.contains("reach") {
            err
        } else {
            "The account session expired. Sign in again, then connect.".to_string()
        };
        set_detail(&message);
        return Err(message);
    }
    let (token, base, signaling, label) = {
        let guard = lock();
        let state = guard.as_ref().ok_or_else(|| "Account is not ready.".to_string())?;
        if state.saved.mesh_token.is_empty() && state.saved.access_token.is_empty() {
            return Err("Sign in before connecting over the internet.".to_string());
        }
        let label = state
            .devices
            .iter()
            .find(|device| device.id == device_id)
            .map(|device| device.name.clone())
            .filter(|name| !name.is_empty())
            .unwrap_or_else(|| "the phone".to_string());
        (
            if state.saved.mesh_token.is_empty() {
                state.saved.access_token.clone()
            } else {
                state.saved.mesh_token.clone()
            },
            state.saved.base_url.clone(),
            state.saved.signaling_url.clone(),
            label,
        )
    };
    set_detail(&format!("Opening an internet tunnel to {label}."));
    let result = cloud::open_tunnel(
        &token,
        &base,
        &signaling,
        name,
        real_id,
        device_id,
        port,
    )
    .await;
    match result {
        Ok(message) => {
            set_detail(&message);
            Ok(view())
        }
        Err(err) => {
            set_detail(&err);
            Err(err)
        }
    }
}

async fn refresh_devices_inner() -> Result<(), String> {
    ensure_mesh().await?;
    let (token, base) = {
        let guard = lock();
        let state = guard.as_ref().ok_or_else(|| "Account is not ready.".to_string())?;
        let token = if state.saved.mesh_token.is_empty() {
            state.saved.access_token.clone()
        } else {
            state.saved.mesh_token.clone()
        };
        (token, effective_base(&state.saved))
    };
    if token.is_empty() {
        return Err("Sign in before loading account devices.".to_string());
    }
    let url = device_list_url(&base);
    let response = http().get(&url).bearer_auth(&token).send().await.map_err(|_| {
        "Could not reach the cloud broker.".to_string()
    })?;
    let status = response.status();
    let body = response.text().await.unwrap_or_default();
    if !status.is_success() {
        return Err(http_failure(status.as_u16(), &body));
    }
    let own = crate::store::load()
        .ok()
        .map(|(identity, _)| cloud::mesh_device_id(&identity.real_id));
    let devices: Vec<AccountDevice> = parse_devices(&body)
        .into_iter()
        .filter(|device| own.as_ref() != Some(&device.id))
        .collect();
    let count = devices.len();
    let mut guard = lock();
    if let Some(state) = guard.as_mut() {
        state.devices = devices;
        let who = if state.saved.name.is_empty() {
            "Signed in".to_string()
        } else {
            format!("Signed in as {}", state.saved.name)
        };
        state.detail = format!(
            "{who}. {count} device{} on this account. Connect uses the internet when the phone has cloud connectivity on. A phone still has to be paired once on Wi-Fi before its screen and messages work.",
            if count == 1 { "" } else { "s" }
        );
    }
    Ok(())
}

async fn store_passport(tokens: Passport) -> Result<(), String> {
    let mut saved = Saved {
        access_token: tokens.access_token,
        refresh_token: tokens.refresh_token,
        access_expires: expires_at(tokens.expires_in),
        name: String::new(),
        ..Saved::default()
    };
    if let Some(st) = fetch_st(&saved.access_token).await {
        saved.lpsust = st;
    }
    if let Some(name) = fetch_name(&saved.access_token).await {
        saved.name = name;
    }
    let access = saved.access_token.clone();
    apply_claims(&mut saved, &access);
    {
        let mut guard = lock();
        let state = guard.as_mut().ok_or_else(|| "Account is not ready.".to_string())?;
        state.saved = saved.clone();
        state.pending = None;
        state.detail = "Signed in. Asking the cloud broker for devices.".to_string();
    }
    if let Ok(path) = pending_path() {
        let _ = fs::remove_file(path);
    }
    write_saved(&saved)?;
    let _ = ensure_mesh().await;
    Ok(())
}

async fn ensure_mesh() -> Result<(), String> {
    let (access, lpsust, mesh, mesh_expires) = {
        let guard = lock();
        let state = guard.as_ref().ok_or_else(|| "Account is not ready.".to_string())?;
        (
            state.saved.access_token.clone(),
            state.saved.lpsust.clone(),
            state.saved.mesh_token.clone(),
            state.saved.mesh_expires,
        )
    };
    if access.is_empty() {
        return Err("Sign in before using the internet connection.".to_string());
    }
    if !mesh.is_empty() && mesh_expires > now() + 60 {
        return Ok(());
    }
    let realm = {
        let guard = lock();
        guard.as_ref().map(|state| state.saved.realm.clone()).unwrap_or_default()
    };
    let subject = if lpsust.is_empty() { access } else { lpsust };
    let exchanged = exchange_mesh(&subject, &realm).await?;
    let mut guard = lock();
    if let Some(state) = guard.as_mut() {
        state.saved.mesh_token = exchanged.token;
        state.saved.mesh_expires = expires_at(exchanged.expires_in);
        let mesh = state.saved.mesh_token.clone();
        apply_claims(&mut state.saved, &mesh);
        let saved = state.saved.clone();
        drop(guard);
        write_saved(&saved)?;
    }
    Ok(())
}

struct Passport {
    access_token: String,
    refresh_token: String,
    expires_in: u64,
}

async fn passport_exchange(code: &str, verifier: &str) -> Result<Passport, String> {
    let body = form(&[
        ("grant_type", "authorization_code"),
        ("code", code),
        ("redirect_uri", REDIRECT),
        ("client_id", CLIENT_ID),
        ("code_verifier", verifier),
    ]);
    let response = http()
        .post(PASSPORT_TOKEN)
        .header("content-type", "application/x-www-form-urlencoded")
        .body(body)
        .send()
        .await
        .map_err(|_| "Could not reach Lenovo sign-in.".to_string())?;
    let status = response.status();
    let text = response.text().await.unwrap_or_default();
    if !status.is_success() {
        return Err(http_failure(status.as_u16(), &text));
    }
    let json: Value = serde_json::from_str(&text).map_err(|_| "Lenovo sign-in returned an unexpected response.".to_string())?;
    let access = first_string(&json, &["access_token", "accessToken", "token"]).ok_or_else(|| {
        "Lenovo sign-in did not return a session.".to_string()
    })?;
    let refresh = first_string(&json, &["refresh_token", "refreshToken"]).unwrap_or_default();
    let expires = json.get("expires_in").and_then(|value| value.as_u64()).unwrap_or(3600);
    Ok(Passport { access_token: access, refresh_token: refresh, expires_in: expires })
}

async fn fetch_st(access: &str) -> Option<String> {
    let response = http().get(ST_URL).bearer_auth(access).header("accessToken", access).send().await.ok()?;
    if !response.status().is_success() {
        return None;
    }
    let text = response.text().await.ok()?;
    let json: Value = serde_json::from_str(&text).ok()?;
    first_string(&json, &["lpsust", "st", "ST", "serviceToken"])
}

async fn fetch_name(access: &str) -> Option<String> {
    let response = http().get(ACCOUNT_INFO).bearer_auth(access).header("accessToken", access).send().await.ok()?;
    if !response.status().is_success() {
        return None;
    }
    let text = response.text().await.ok()?;
    let json: Value = serde_json::from_str(&text).ok()?;
    first_string(&json, &["nickname", "Username", "userName", "alias", "name"])
}

struct MeshToken {
    token: String,
    expires_in: u64,
}

async fn finish_wust(session: WustSession) -> Result<AccountView, String> {
    set_detail("Signing in with the Lenovo session from the browser.");
    {
        let mut guard = lock();
        if let Some(state) = guard.as_mut() {
            state.busy = true;
        }
    }
    let name = match fetch_account_by_lpsust(&session.token, &session.realm).await {
        Ok(name) => name,
        Err(err) => {
            clear_busy();
            return Err(err);
        }
    };
    let saved = Saved {
        // The passport callback returns this session ticket instead of an OAuth code.
        access_token: session.token.clone(),
        lpsust: session.token,
        name,
        realm: session.realm,
        ..Saved::default()
    };
    {
        let mut guard = lock();
        let state = match guard.as_mut() {
            Some(state) => state,
            None => {
                clear_busy();
                return Err("Account is not ready.".to_string());
            }
        };
        state.saved = saved.clone();
        state.pending = None;
        state.detail = "Signed in. Asking the cloud broker for devices.".to_string();
    }
    if let Err(err) = write_saved(&saved) {
        clear_busy();
        return Err(err);
    }
    if let Ok(path) = pending_path() {
        let _ = fs::remove_file(path);
    }
    if let Err(err) = refresh_devices_inner().await {
        set_detail(&format!("Signed in. The account device list is not available yet: {err}"));
    }
    clear_busy();
    Ok(view())
}

async fn fetch_account_by_lpsust(lpsust: &str, realm: &str) -> Result<String, String> {
    let mut url = format!("{ACCOUNT_INFO}?lpsust={}", encode(lpsust));
    if !realm.is_empty() {
        url.push_str("&realm=");
        url.push_str(&encode(realm));
    }
    let response = http()
        .get(&url)
        .send()
        .await
        .map_err(|_| "Could not reach Lenovo sign-in.".to_string())?;
    let status = response.status();
    let text = response.text().await.unwrap_or_default();
    if !status.is_success() {
        return Err(http_failure(status.as_u16(), &text));
    }
    let json: Value = serde_json::from_str(&text).unwrap_or(Value::Null);
    Ok(first_string(&json, &["nickname", "Username", "userName", "alias", "name"]).unwrap_or_default())
}

struct WustSession {
    token: String,
    realm: String,
}

/// A passport callback carries `lenovoid.wust` when the custom scheme did not open.
fn wust_from(url: &str) -> Option<WustSession> {
    let mut token = None;
    let mut realm = String::new();
    for (key, value) in query_pairs(url) {
        match key.as_str() {
            "lenovoid.wust" | "wust" | "lpsust" if value.len() >= 16 => token = Some(value),
            "lenovoid.realm" | "realm" if !value.is_empty() => realm = value,
            _ => {}
        }
    }
    Some(WustSession { token: token?, realm })
}

async fn exchange_mesh(subject: &str, realm: &str) -> Result<MeshToken, String> {
    let mut last = "The cloud session was not accepted.".to_string();
    for host in KEYCLOAKS {
        let url = format!("{host}/auth/realms/LenovoConsumer/protocol/openid-connect/token");
        for body in mesh_grants(subject, realm) {
            let response = match http()
                .post(&url)
                .header("content-type", "application/x-www-form-urlencoded")
                .body(body)
                .send()
                .await
            {
                Ok(response) => response,
                Err(_) => {
                    last = "Could not reach the account service.".to_string();
                    continue;
                }
            };
            let status = response.status();
            let text = response.text().await.unwrap_or_default();
            if !status.is_success() {
                last = http_failure(status.as_u16(), &text);
                continue;
            }
            if let Ok(json) = serde_json::from_str::<Value>(&text) {
                if let Some(token) = first_string(&json, &["access_token", "accessToken"]) {
                    let expires = json.get("expires_in").and_then(|value| value.as_u64()).unwrap_or(3600);
                    return Ok(MeshToken { token, expires_in: expires });
                }
            }
            last = "The account service returned an unexpected session.".to_string();
        }
    }
    Err(last)
}

fn mesh_grants(subject: &str, realm: &str) -> Vec<String> {
    let mut grants = Vec::new();
    let mut realms = vec!["LenovoConsumer".to_string()];
    if !realm.is_empty() && realm != "LenovoConsumer" {
        realms.push(realm.to_string());
    }
    for realm in realms {
        grants.push(form(&[
            ("grant_type", "urn:ietf:params:oauth:grant-type:token-exchange"),
            ("client_id", MESH_CLIENT),
            ("subject_token", subject),
            ("subject_token_type", "urn:ietf:params:oauth:token-type:access_token"),
            ("subject_issuer", "https://passport.lenovo.com"),
            ("subject_realm", &realm),
        ]));
    }
    grants.push(form(&[
        ("grant_type", "urn:ietf:params:oauth:grant-type:jwt-bearer"),
        ("client_id", MESH_CLIENT),
        ("assertion", subject),
    ]));
    grants
}

fn apply_claims(saved: &mut Saved, token: &str) {
    let Some(claims) = jwt_claims(token) else {
        return;
    };
    if let Some(base) = first_string(&claims, &["sdk_base_uri", "base_url", "serverUrl", "auth_base_uri"]) {
        if base.starts_with("https://") {
            saved.base_url = base;
        }
    }
    if let Some(signaling) = first_string(&claims, &["signaling_url", "signalingUrl", "serverUrl"]) {
        if signaling.starts_with("https://") || signaling.starts_with("wss://") {
            saved.signaling_url = signaling;
        }
    }
}

fn effective_base(saved: &Saved) -> String {
    if saved.base_url.starts_with("https://") {
        saved.base_url.trim_end_matches('/').to_string()
    } else {
        "https://cloudbroker.uds.lenovo.com".to_string()
    }
}

/// REST base the mesh client uses: the SDK host plus `/signaling-service`.
pub(crate) fn service_base(saved_base: &str) -> String {
    let base = if saved_base.starts_with("https://") {
        saved_base.trim_end_matches('/').to_string()
    } else {
        "https://cloudbroker.uds.lenovo.com".to_string()
    };
    if base.ends_with("/signaling-service") {
        base
    } else {
        format!("{base}/signaling-service")
    }
}

pub(crate) fn device_list_url(saved_base: &str) -> String {
    format!("{}/devices", service_base(saved_base))
}

/// SDK host only. The signaling REST prefix is added by [`service_base`].
pub(crate) fn sdk_origin(saved_base: &str) -> String {
    let base = if saved_base.starts_with("https://") {
        saved_base.trim_end_matches('/').to_string()
    } else {
        "https://sdk-services.uds.lenovo.com".to_string()
    };
    base.trim_end_matches("/signaling-service").to_string()
}

pub(crate) fn policy_url(saved_base: &str) -> String {
    format!(
        "{}/product-config/regional/products/mesh/policy-v2.jwt",
        sdk_origin(saved_base)
    )
}

/// Notification socket the mesh client listens on. `channels` is already comma-separated.
pub(crate) fn nrt_websocket_url(saved_base: &str, channels: &str) -> String {
    let host = sdk_origin(saved_base).trim_start_matches("https://").to_string();
    format!(
        "wss://{host}/api/notifications/v1/ws?channels={}",
        query_component(channels)
    )
}

pub(crate) fn channels_from_policy(jwt: &str) -> Vec<String> {
    let Some(payload) = jwt.split('.').nth(1) else {
        return Vec::new();
    };
    let Ok(bytes) = URL_SAFE_NO_PAD.decode(payload.trim().trim_end_matches('=')) else {
        return Vec::new();
    };
    let Ok(json) = serde_json::from_slice::<Value>(&bytes) else {
        return Vec::new();
    };
    let Some(products) = json.get("products").and_then(|value| value.as_array()) else {
        return Vec::new();
    };
    let mut fallback = Vec::new();
    for item in products {
        if item.get("id").and_then(|value| value.as_str()) != Some("smartconnect-pc") {
            continue;
        }
        let ids = channel_ids(item);
        if ids.is_empty() {
            continue;
        }
        let versions = item.get("appliesToSdkVersions").and_then(|value| value.as_array());
        let wants_this_sdk = versions.is_some_and(|list| {
            list.iter().any(|version| version.as_str() == Some("v0.3.455"))
        });
        if wants_this_sdk {
            return ids;
        }
        if fallback.is_empty() {
            fallback = ids;
        }
    }
    fallback
}

fn channel_ids(item: &Value) -> Vec<String> {
    item.get("config")
        .and_then(|config| config.get("notificationChannelIDs"))
        .and_then(|value| value.as_array())
        .map(|list| {
            list.iter()
                .filter_map(|value| value.as_str())
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .map(str::to_string)
                .collect()
        })
        .unwrap_or_default()
}

fn query_component(value: &str) -> String {
    let mut out = String::new();
    for byte in value.bytes() {
        match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' | b',' => {
                out.push(byte as char);
            }
            _ => out.push_str(&format!("%{byte:02X}")),
        }
    }
    out
}

fn parse_devices(body: &str) -> Vec<AccountDevice> {
    let Ok(json) = serde_json::from_str::<Value>(body) else {
        return Vec::new();
    };
    let list = json
        .get("devices")
        .or_else(|| json.get("deviceList"))
        .cloned()
        .or_else(|| if json.is_array() { Some(json) } else { None });
    let Some(Value::Array(items)) = list else {
        return Vec::new();
    };
    items
        .iter()
        .filter_map(|item| {
            let id = first_string(item, &["realDeviceId", "deviceId", "id"])?;
            let name = first_string(item, &["deviceName", "name", "scDeviceName"]).unwrap_or_else(|| "Phone".to_string());
            // The signaling list has deviceId and lastSeenTimestamp, not an online flag.
            let online = item.get("online").and_then(|value| value.as_bool()).unwrap_or(false);
            Some(AccountDevice { id, name, online })
        })
        .collect()
}

fn first_string(value: &Value, keys: &[&str]) -> Option<String> {
    for key in keys {
        if let Some(text) = value.get(*key).and_then(|item| item.as_str()) {
            let text = text.trim();
            if !text.is_empty() {
                return Some(text.to_string());
            }
        }
    }
    if let Some(data) = value.get("data") {
        if data.is_object() || data.is_array() {
            return first_string(data, keys);
        }
    }
    None
}

fn jwt_claims(token: &str) -> Option<Value> {
    let payload = token.split('.').nth(1)?;
    let bytes = URL_SAFE_NO_PAD.decode(payload).ok()?;
    serde_json::from_slice(&bytes).ok()
}

fn authorize_url(challenge: &str, state: &str) -> String {
    format!(
        "{AUTHORIZE}?client_id={CLIENT_ID}&response_type=code&redirect_uri={}&state={}&code_challenge={}&code_challenge_method=S256",
        encode(REDIRECT),
        encode(state),
        encode(challenge),
    )
}

fn pkce_challenge(verifier: &str) -> String {
    URL_SAFE_NO_PAD.encode(Sha256::digest(verifier.as_bytes()))
}

fn query_pairs(url: &str) -> Vec<(String, String)> {
    let query = url.split_once('?').map(|(_, query)| query).unwrap_or(url);
    let mut pairs = Vec::new();
    for pair in query.split(['&', '#']) {
        if pair.is_empty() {
            continue;
        }
        let mut parts = pair.splitn(2, '=');
        let key = decode_component(parts.next().unwrap_or(""));
        let value = decode_component(parts.next().unwrap_or(""));
        pairs.push((key, value));
    }
    pairs
}

fn callback_parts(url: &str) -> Option<(String, String)> {
    let mut code = None;
    let mut state = None;
    for (key, value) in query_pairs(url) {
        match key.as_str() {
            "code" if !value.is_empty() => code = Some(value),
            "state" => state = Some(value),
            _ => {}
        }
    }
    Some((code?, state.unwrap_or_default()))
}

/// `Ok(None)` means another callback already finished, or is still finishing.
fn take_pending(state: &str) -> Result<Option<Pending>, String> {
    let mut guard = lock();
    let slot = guard.as_mut().ok_or_else(|| "Account is not ready.".to_string())?;
    if slot.pending.is_none() {
        slot.pending = pending_path().ok().and_then(|path| read_secret(&path));
    }
    let Some(pending) = slot.pending.take() else {
        if slot.busy || !slot.saved.access_token.is_empty() {
            return Ok(None);
        }
        return Err("Sign-in expired. Start again.".to_string());
    };
    if !state.is_empty() && pending.state != state {
        slot.pending = Some(pending);
        return Err("Sign-in did not match the request that was started. Start again.".to_string());
    }
    slot.busy = true;
    Ok(Some(pending))
}

fn clear_busy() {
    if let Some(state) = lock().as_mut() {
        state.busy = false;
    }
}

fn http_failure(status: u16, body: &str) -> String {
    if let Ok(json) = serde_json::from_str::<Value>(body) {
        if let Some(error) = json.get("error").and_then(|value| value.as_str()) {
            let description = json.get("error_description").and_then(|value| value.as_str()).unwrap_or("");
            let description = scrub(description);
            if description.is_empty() {
                return format!("{error} ({status})");
            }
            return format!("{error}: {description} ({status})");
        }
    }
    format!("The account service rejected the request ({status}).")
}

fn scrub(text: &str) -> String {
    let mut out = String::new();
    for word in text.split_whitespace().take(24) {
        if word.len() > 48 || word.contains("eyJ") {
            continue;
        }
        if !out.is_empty() {
            out.push(' ');
        }
        out.push_str(word);
    }
    out
}

fn set_detail(detail: &str) {
    let emit = {
        let mut guard = lock();
        let Some(state) = guard.as_mut() else {
            return;
        };
        state.detail = detail.to_string();
        state.app.clone().map(|app| (app, state.view()))
    };
    if let Some((app, view)) = emit {
        let _ = app.emit("account", &view);
    }
}

fn failed_view(detail: &str) -> AccountView {
    set_detail(detail);
    view()
}

impl State {
    fn view(&self) -> AccountView {
        AccountView {
            signed_in: !self.saved.access_token.is_empty(),
            name: self.saved.name.clone(),
            detail: self.detail.clone(),
            devices: self.devices.clone(),
        }
    }
}

fn idle_detail() -> String {
    "Sign in with the same Lenovo or Motorola account as the phone to connect across networks. The phone needs cloud connectivity on. Wi-Fi and Bluetooth wake keep working either way.".to_string()
}

fn form(pairs: &[(&str, &str)]) -> String {
    pairs
        .iter()
        .map(|(key, value)| format!("{}={}", encode(key), encode(value)))
        .collect::<Vec<_>>()
        .join("&")
}

fn encode(value: &str) -> String {
    let mut out = String::new();
    for byte in value.bytes() {
        match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => out.push(byte as char),
            _ => out.push_str(&format!("%{byte:02X}")),
        }
    }
    out
}

fn decode_component(value: &str) -> String {
    let bytes = value.replace('+', " ");
    let mut out = Vec::new();
    let raw = bytes.as_bytes();
    let mut index = 0;
    while index < raw.len() {
        if raw[index] == b'%' && index + 2 < raw.len() {
            if let Ok(byte) = u8::from_str_radix(&bytes[index + 1..index + 3], 16) {
                out.push(byte);
                index += 3;
                continue;
            }
        }
        out.push(raw[index]);
        index += 1;
    }
    String::from_utf8_lossy(&out).into_owned()
}

fn random_url(bytes: usize) -> String {
    let mut buffer = vec![0u8; bytes];
    getrandom_fill(&mut buffer);
    URL_SAFE_NO_PAD.encode(buffer)
}

fn getrandom_fill(buffer: &mut [u8]) {
    // rand 0.8 is already a dependency and is enough for a PKCE verifier.
    use rand::RngCore;
    rand::thread_rng().fill_bytes(buffer);
}

fn expires_at(seconds: u64) -> u64 {
    now().saturating_add(seconds)
}

fn now() -> u64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map(|duration| duration.as_secs()).unwrap_or(0)
}

fn http() -> reqwest::Client {
    reqwest::Client::builder().timeout(std::time::Duration::from_secs(25)).build().unwrap_or_else(|_| reqwest::Client::new())
}

fn lock() -> std::sync::MutexGuard<'static, Option<State>> {
    STATE.lock().unwrap_or_else(|poison| poison.into_inner())
}

fn support_dir() -> Result<PathBuf, String> {
    let base = dirs::data_dir().ok_or_else(|| "No Application Support directory.".to_string())?;
    let dir = base.join("MotoSmartConnect");
    fs::create_dir_all(&dir).map_err(|_| "Could not create the account folder.".to_string())?;
    Ok(dir)
}

fn token_path() -> Result<PathBuf, String> {
    Ok(support_dir()?.join("account.json"))
}

fn pending_path() -> Result<PathBuf, String> {
    Ok(support_dir()?.join("account-pending.json"))
}

fn read_saved() -> Saved {
    token_path().ok().and_then(|path| read_secret(&path)).unwrap_or_default()
}

fn read_secret<T: for<'de> Deserialize<'de>>(path: &PathBuf) -> Option<T> {
    let text = fs::read_to_string(path).ok()?;
    serde_json::from_str(&text).ok()
}

fn write_saved(saved: &Saved) -> Result<(), String> {
    write_secret(&token_path()?, saved)
}

fn write_secret<T: Serialize>(path: &PathBuf, value: &T) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|_| "Could not save the account.".to_string())?;
    }
    let text = serde_json::to_string(value).map_err(|_| "Could not save the account.".to_string())?;
    let mut file = OpenOptions::new()
        .write(true)
        .create(true)
        .truncate(true)
        .mode(0o600)
        .open(path)
        .map_err(|_| "Could not save the account.".to_string())?;
    file.write_all(text.as_bytes()).map_err(|_| "Could not save the account.".to_string())?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pkce_challenge_is_s256_without_padding() {
        let challenge = pkce_challenge("verifier-value");
        assert_eq!(challenge, URL_SAFE_NO_PAD.encode(Sha256::digest(b"verifier-value")));
        assert!(!challenge.contains('='));
    }

    #[test]
    fn authorize_url_carries_the_challenge_and_not_the_verifier() {
        let url = authorize_url("challenge-value", "state-value");
        assert!(url.contains("code_challenge=challenge-value"));
        assert!(url.contains("code_challenge_method=S256"));
        assert!(url.contains("redirect_uri=smartconnect%3Aauthcode"));
        assert!(!url.contains("verifier"));
        assert!(url.starts_with(AUTHORIZE));
    }

    #[test]
    fn callback_reads_the_code_and_state() {
        let (code, state) = callback_parts("smartconnect:authcode?code=abc%2B&state=state-value").unwrap();
        assert_eq!(code, "abc+");
        assert_eq!(state, "state-value");
    }

    #[test]
    fn passport_callback_reads_the_session_and_realm() {
        let session = wust_from(
            "https://passport.lenovo.com/v1.0/utility/lenovoid/oauth2/callback?lenovoid.wust=session-ticket-value&lenovoid.action=uilogin&lenovoid.realm=com.motorola.sc.pcdesktop&lenovoid.ctx=ctx",
        )
        .unwrap();
        assert_eq!(session.token, "session-ticket-value");
        assert_eq!(session.realm, "com.motorola.sc.pcdesktop");
        assert!(wust_from("smartconnect:authcode?code=abc&state=state-value").is_none());
    }

    #[test]
    fn device_list_uses_the_signaling_service() {
        assert_eq!(
            device_list_url("https://sdk-services.example.test"),
            "https://sdk-services.example.test/signaling-service/devices"
        );
        assert_eq!(
            device_list_url("https://sdk-services.example.test/signaling-service/"),
            "https://sdk-services.example.test/signaling-service/devices"
        );
        let devices = parse_devices(
            r#"{"devices":[{"deviceId":"phone-1","lastSeenTimestamp":"2026-01-01T00:00:00Z"}]}"#,
        );
        assert_eq!(devices.len(), 1);
        assert_eq!(devices[0].id, "phone-1");
        assert_eq!(devices[0].name, "Phone");
        assert!(!devices[0].online);
    }

    #[test]
    fn nrt_socket_uses_the_notification_path() {
        assert_eq!(
            nrt_websocket_url("https://sdk-services.example.test", "channel-1"),
            "wss://sdk-services.example.test/api/notifications/v1/ws?channels=channel-1"
        );
        assert_eq!(
            policy_url("https://sdk-services.example.test/signaling-service"),
            "https://sdk-services.example.test/product-config/regional/products/mesh/policy-v2.jwt"
        );
        let payload = URL_SAFE_NO_PAD.encode(
            br#"{"products":[{"id":"smartconnect-pc","appliesToSdkVersions":["v0.4.473"],"config":{"notificationChannelIDs":["other-channel"]}},{"id":"smartconnect-pc","appliesToSdkVersions":["v0.3.455"],"config":{"notificationChannelIDs":["channel-1"]}}]}"#,
        );
        let channels = channels_from_policy(&format!("e30.{payload}.sig"));
        assert_eq!(channels, vec!["channel-1".to_string()]);
    }

    #[test]
    fn error_text_drops_token_like_words() {
        let text = scrub("invalid_grant eyJhbGciOiJub25lIn0.payload.sig short");
        assert!(!text.contains("eyJ"));
        assert!(text.contains("invalid_grant"));
        assert!(text.contains("short"));
    }
}
