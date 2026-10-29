//! Phone features on an already paired control connection.
//!
//! Message shapes follow the public protocol notes in smartconnect-linux (MIT):
//! notifications, clipboard text, file pulls, app streaming, screen mirroring,
//! and cross control. Hotspot is not handled.

use base64::Engine;
use serde::Serialize;
use serde_json::{json, Value};
use std::collections::{HashMap, HashSet, VecDeque};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

const NOTICE_LIMIT: usize = 40;
const FILE_LIMIT: usize = 30;

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ActionView {
    pub id: i64,
    pub name: String,
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NoticeView {
    pub key: String,
    pub app: String,
    pub pkg: String,
    pub title: String,
    pub content: String,
    pub can_reply: bool,
    pub actions: Vec<ActionView>,
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AppView {
    pub pkg: String,
    pub name: String,
    pub cmp: String,
    pub user_id: i64,
    pub icon_name: String,
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FileView {
    pub name: String,
    pub path: String,
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionView {
    pub session_id: String,
    pub title: String,
    pub pkg: String,
    pub display_id: i64,
    pub width: i64,
    pub height: i64,
    pub quiet: bool,
    pub support_touch: bool,
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CrossView {
    pub active: bool,
    pub width: i64,
    pub height: i64,
}

#[derive(Clone)]
struct Notice {
    service: i64,
    app: String,
    pkg: String,
    title: String,
    content: String,
    can_reply: bool,
    actions: Vec<ActionView>,
    fingerprint: String,
    shown_at: Instant,
}

#[derive(Clone)]
struct Session {
    pkg: String,
    title: String,
    user_id: i64,
    display_id: i64,
    width: i64,
    height: i64,
    quiet: bool,
    saw_offer: bool,
    support_touch: bool,
}

struct Awaiting {
    pkg: String,
    quiet: bool,
    until: Instant,
}

#[derive(Clone)]
pub struct PullJob {
    pub port: u16,
    pub fingerprint: String,
    pub token: String,
    pub icons: bool,
}

pub enum Effect {
    Send { service: i64, data: Value },
    Note(String),
    Publish,
    Banner { title: String, body: String },
    Paste(String),
    Pull(PullJob),
    Offer {
        session_id: String,
        sdp: String,
        quiet: bool,
        pkg: String,
    },
    Ice {
        session_id: String,
        candidate: String,
        label: i64,
        mid: String,
    },
    Closed { session_id: String },
    /// Open or close OBS. Only the phone-camera button turns this on.
    Obs { on: bool },
    CloseLater { session_id: String, secs: u64 },
    Status,
    /// The phone has 20 seconds to answer a start. The engine clears a start that never does.
    WatchStart { token: u64 },
    /// A service 168 reply named the phone's folder server.
    ShareReady,
}

enum StartGate {
    /// Continue the start. Effects are a cancel of an older attempt, or empty.
    Go(Vec<Effect>),
    /// A start is only a moment old. Leave it alone.
    Wait(Vec<Effect>),
}

pub struct Desk {
    pub clipboard_ready: bool,
    pub cross_active: bool,
    pub cross_width: i64,
    pub cross_height: i64,
    pub icons_revision: u64,
    pub instance_id: i64,
    notifications: VecDeque<(String, Notice)>,
    apps: Vec<AppView>,
    icons: HashMap<String, String>,
    files: VecDeque<FileView>,
    sessions: HashMap<String, Session>,
    visible: Option<String>,
    last_clip: Option<String>,
    clip_seq: i64,
    next_cid: i64,
    cross_cid: i64,
    awaiting: Option<Awaiting>,
    starting: bool,
    start_at: Instant,
    start_token: u64,
    /// Package of a start this Mac just sent, so a phone echo does not send it twice.
    opened_at: Option<(String, Instant)>,
    caps_sent: HashMap<i64, bool>,
    suppress_close: HashMap<String, bool>,
    pending_quiet: HashMap<String, bool>,
    pending_title: HashMap<String, String>,
    station_noted: bool,
    phone_share: Option<crate::nfs::PhoneShare>,
    share_gen: u64,
    banner_failed: bool,
    ice_shape_noted: bool,
    ice_seen_noted: bool,
    /// A camera request is out, or the phone asked, before its offer arrives.
    webcam_armed: bool,
}

impl Desk {
    pub fn new() -> Self {
        let secs = SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(1);
        Self {
            clipboard_ready: false,
            cross_active: false,
            cross_width: 0,
            cross_height: 0,
            icons_revision: 0,
            instance_id: 1_700_000_000_000 + (secs as i64 % 1_000_000_000),
            notifications: VecDeque::new(),
            apps: Vec::new(),
            icons: HashMap::new(),
            files: VecDeque::new(),
            sessions: HashMap::new(),
            visible: None,
            last_clip: None,
            clip_seq: (secs as i64).max(1),
            next_cid: 1,
            cross_cid: 0,
            awaiting: None,
            starting: false,
            start_at: Instant::now(),
            start_token: 0,
            opened_at: None,
            caps_sent: HashMap::new(),
            suppress_close: HashMap::new(),
            pending_quiet: HashMap::new(),
            pending_title: HashMap::new(),
            station_noted: false,
            phone_share: None,
            share_gen: 0,
            banner_failed: false,
            ice_shape_noted: false,
            ice_seen_noted: false,
            webcam_armed: false,
        }
    }

    pub fn on_connected(&mut self) -> Vec<Effect> {
        self.clipboard_ready = false;
        self.cross_active = false;
        self.starting = false;
        self.opened_at = None;
        self.ice_shape_noted = false;
        self.ice_seen_noted = false;
        let stop_obs = self.webcam_armed || self.sessions.values().any(|session| session.pkg == "webcam");
        self.webcam_armed = false;
        self.caps_sent.clear();
        self.phone_share = None;
        let ids: Vec<String> = self.sessions.keys().cloned().collect();
        self.sessions.clear();
        self.visible = None;
        let mut effects: Vec<Effect> = ids.into_iter().map(|session_id| Effect::Closed { session_id }).collect();
        if stop_obs {
            effects.push(Effect::Obs { on: false });
        }
        effects
    }

    pub fn handle(&mut self, service: i64, data: &Value) -> Vec<Effect> {
        match service {
            1 => self.on_view(data),
            2 => self.on_fex(data),
            5 | 37 => self.on_notification(service, data),
            9 => self.on_clipboard(data),
            3 => self.on_webcam(data),
            15 => self.on_stream(data),
            17 => self.on_apps(data),
            26 => self.on_cross(data),
            168 => self.on_station(data),
            _ => Vec::new(),
        }
    }

    pub fn notifications(&self) -> Vec<NoticeView> {
        self.notifications
            .iter()
            .map(|(key, notice)| NoticeView {
                key: key.clone(),
                app: notice.app.clone(),
                pkg: notice.pkg.clone(),
                title: notice.title.clone(),
                content: notice.content.clone(),
                can_reply: notice.can_reply,
                actions: notice.actions.clone(),
            })
            .collect()
    }

    pub fn apps(&self) -> Vec<AppView> {
        self.apps.clone()
    }

    pub fn icons(&self) -> HashMap<String, String> {
        self.icons.clone()
    }

    pub fn files(&self) -> Vec<FileView> {
        self.files.iter().cloned().collect()
    }

    pub fn sessions(&self) -> Vec<SessionView> {
        let mut views: Vec<SessionView> = self
            .sessions
            .iter()
            .map(|(id, session)| SessionView {
                session_id: id.clone(),
                title: session.title.clone(),
                pkg: session.pkg.clone(),
                display_id: session.display_id,
                width: session.width,
                height: session.height,
                quiet: session.quiet,
                support_touch: session.support_touch,
            })
            .collect();
        views.sort_by(|left, right| left.title.cmp(&right.title));
        views
    }

    pub fn session_pkg(&self, session_id: &str) -> String {
        self.sessions.get(session_id).map(|session| session.pkg.clone()).unwrap_or_default()
    }

    pub fn visible(&self) -> Option<SessionView> {
        let id = self.visible.as_ref()?;
        let session = self.sessions.get(id)?;
        Some(SessionView {
            session_id: id.clone(),
            title: session.title.clone(),
            pkg: session.pkg.clone(),
            display_id: session.display_id,
            width: session.width,
            height: session.height,
            quiet: session.quiet,
            support_touch: session.support_touch,
        })
    }

    pub fn cross(&self) -> CrossView {
        CrossView {
            active: self.cross_active,
            width: self.cross_width,
            height: self.cross_height,
        }
    }

    pub fn has_sessions(&self) -> bool {
        self.sessions.values().any(|session| session.pkg != "webcam")
    }

    pub fn webcam_live(&self) -> bool {
        self.sessions.values().any(|session| session.pkg == "webcam")
    }

    /// Heartbeats start at the request. The phone drops the camera after 30 seconds of silence.
    pub fn webcam_heartbeat(&self) -> bool {
        self.webcam_armed || self.webcam_live()
    }

    pub fn mark_banner_failed(&mut self) -> bool {
        if self.banner_failed {
            return false;
        }
        self.banner_failed = true;
        true
    }

    pub fn clipboard_out(&mut self, text: &str) -> Option<Value> {
        if !self.clipboard_ready || text.is_empty() || self.last_clip.as_deref() == Some(text) {
            return None;
        }
        let text = clip_text(text);
        self.last_clip = Some(text.clone());
        self.clip_seq = self.clip_seq.saturating_add(1).max(1);
        Some(json!({ "msgId": 3, "data": { "clipId": self.clip_seq, "text": text } }))
    }

    pub fn reply(&mut self, key: &str, text: &str) -> Result<Vec<Effect>, String> {
        let text = text.trim();
        if text.is_empty() {
            return Err("Write a reply first.".into());
        }
        if text.chars().count() > 2000 {
            return Err("Keep the reply under 2000 characters.".into());
        }
        let notice = self.notifications.iter().find(|(id, _)| id == key).map(|(_, notice)| notice.clone());
        let Some(notice) = notice else {
            return Err("That notification is already gone.".into());
        };
        if !notice.can_reply {
            return Err("That notification has no reply.".into());
        }
        Ok(vec![
            Effect::Send {
                service: notice.service,
                data: json!({
                    "cmd": 0,
                    "event": 0,
                    "key": key,
                    "version": "1.0",
                    "data": { "reply_content": text, "smart_reply": false }
                }),
            },
            Effect::Note("Sent a reply.".into()),
        ])
    }

    pub fn act(&mut self, key: &str, action_id: i64) -> Result<Vec<Effect>, String> {
        let notice = self.notifications.iter().find(|(id, _)| id == key).map(|(_, notice)| notice.clone());
        let Some(notice) = notice else {
            return Err("That notification is already gone.".into());
        };
        if !notice.actions.iter().any(|action| action.id == action_id) {
            return Err("That button is not on the notification.".into());
        }
        self.awaiting = Some(Awaiting {
            pkg: notice.pkg.clone(),
            quiet: true,
            until: Instant::now() + Duration::from_secs(8),
        });
        Ok(vec![
            Effect::Send {
                service: notice.service,
                data: json!({
                    "cmd": 0,
                    "event": 1,
                    "key": key,
                    "version": "1.0",
                    "data": { "action_id": action_id }
                }),
            },
            Effect::Note("Sent the button to the phone.".into()),
        ])
    }

    pub fn open_notice(&mut self, key: &str) -> Result<Vec<Effect>, String> {
        let notice = self.notifications.iter().find(|(id, _)| id == key).map(|(_, notice)| notice.clone());
        let Some(notice) = notice else {
            return Err("That notification is already gone.".into());
        };
        self.awaiting = Some(Awaiting {
            pkg: notice.pkg.clone(),
            quiet: false,
            until: Instant::now() + Duration::from_secs(8),
        });
        let mut effects = vec![
            Effect::Send {
                service: notice.service,
                data: json!({
                    "cmd": 0,
                    "event": 2,
                    "key": key,
                    "version": "1.0"
                }),
            },
            Effect::Note("Asked the phone to open it.".into()),
            Effect::Publish,
        ];
        if !notice.pkg.is_empty() {
            let found = self.apps.iter().find(|app| app.pkg == notice.pkg);
            let cmp = found.map(|app| app.cmp.clone()).unwrap_or_default();
            let user_id = found.map(|app| app.user_id).unwrap_or(0);
            match self.start_app(&notice.pkg, &cmp, user_id) {
                Ok(more) => effects.extend(more),
                Err(err) => effects.push(Effect::Note(err)),
            }
        }
        Ok(effects)
    }

    pub fn start_mirror(&mut self) -> Result<Vec<Effect>, String> {
        if let Some(id) = self
            .sessions
            .iter()
            .find(|(_, session)| session.pkg == "mirror" && !session.quiet)
            .map(|(id, _)| id.clone())
        {
            self.visible = Some(id);
            return Ok(vec![Effect::Note("The phone screen is already open.".into()), Effect::Publish]);
        }
        let mut effects = match self.gate_start() {
            StartGate::Wait(effects) => return Ok(effects),
            StartGate::Go(effects) => effects,
        };
        let token = self.begin_start();
        self.remember_open("mirror");
        // The phone waits for both messages. Service 1 only says a view is starting.
        // Service 15 cmd 1 with source 0 is what makes the phone send the picture offer.
        effects.extend([
            Effect::Send {
                service: 1,
                data: json!({ "ver": "1.0.0", "cmd": 1, "view": 1, "reply": 0, "drawerMode": false }),
            },
            Effect::Send { service: 15, data: view_start("mirror", self.instance_id) },
            Effect::Note("Asking the phone to share its screen.".into()),
            Effect::Publish,
            Effect::WatchStart { token },
        ]);
        Ok(effects)
    }

    pub fn start_desktop(&mut self) -> Result<Vec<Effect>, String> {
        if self.sessions.values().any(|session| session.pkg == "desktop" && !session.quiet) {
            return Ok(vec![Effect::Note("The phone desktop is already open.".into()), Effect::Publish]);
        }
        let mut effects = match self.gate_start() {
            StartGate::Wait(effects) => return Ok(effects),
            StartGate::Go(effects) => effects,
        };
        let token = self.begin_start();
        self.remember_open("desktop");
        effects.extend([
            Effect::Send {
                service: 1,
                data: json!({ "ver": "1.0.0", "cmd": 1, "view": 2, "reply": 0, "drawerMode": false }),
            },
            Effect::Send { service: 15, data: view_start("desktop", self.instance_id) },
            Effect::Note("Asking the phone for its desktop.".into()),
            Effect::Publish,
            Effect::WatchStart { token },
        ]);
        Ok(effects)
    }

    /// Ask the phone to serve its storage. `fp_hex` is this Mac's certificate.
    pub fn phone_browse_request(fp_hex: &str) -> Value {
        let inner = json!({
            "type": 0,
            "dt2type": 2,
            "nfs_dependency": 8,
            "fp": fp_hex.trim().to_ascii_uppercase(),
        });
        json!({
            "ver": "1.0",
            "cmd": 1001,
            "data": inner.to_string(),
        })
    }

    pub fn share_generation(&self) -> u64 {
        self.share_gen
    }

    pub fn phone_share(&self) -> Option<crate::nfs::PhoneShare> {
        self.phone_share.clone()
    }

    pub fn clear_share(&mut self) {
        self.phone_share = None;
    }

    pub fn start_app(&mut self, pkg: &str, cmp: &str, user_id: i64) -> Result<Vec<Effect>, String> {
        let pkg = pkg.trim();
        if pkg.is_empty() {
            return Err("Choose an app.".into());
        }
        let mut effects = match self.gate_start() {
            StartGate::Wait(effects) => return Ok(effects),
            StartGate::Go(effects) => effects,
        };
        let already = self
            .sessions
            .iter()
            .find(|(_, session)| session.pkg == pkg && !session.quiet)
            .map(|(id, session)| (id.clone(), session.pkg.clone()));
        if let Some((id, found)) = already {
            if found == "mirror" {
                self.visible = Some(id);
            }
            effects.push(Effect::Note(format!("{pkg} is already open.")));
            effects.push(Effect::Publish);
            return Ok(effects);
        }
        let token = self.begin_start();
        self.remember_open(pkg);
        let name = self
            .apps
            .iter()
            .find(|app| app.pkg == pkg && app.user_id == user_id)
            .map(|app| app.name.clone())
            .unwrap_or_else(|| pkg.to_string());
        effects.extend([
            Effect::Send { service: 15, data: stream_start(pkg, cmp, user_id, 0, self.instance_id) },
            Effect::Note(format!("Opening {name}.")),
            Effect::Publish,
            Effect::WatchStart { token },
        ]);
        Ok(effects)
    }

    pub fn start_webcam(&mut self) -> Result<Vec<Effect>, String> {
        if self.webcam_live() || self.webcam_armed {
            return Ok(vec![
                Effect::Note("The phone camera is already on.".into()),
                Effect::Obs { on: true },
                Effect::Publish,
            ]);
        }
        self.webcam_armed = true;
        Ok(vec![
            // No isReply key. The phone treats a present key as its own reply, even when the value is 0.
            Effect::Send { service: 3, data: webcam_request() },
            Effect::Note("Asking the phone to share its camera.".into()),
            Effect::Note("Other apps can choose OBS Virtual Camera while this camera is on.".into()),
            Effect::Obs { on: true },
            Effect::Publish,
        ])
    }

    pub fn webcam_flip(&mut self) -> Result<Vec<Effect>, String> {
        if !self.webcam_live() {
            return Err("The phone camera is not on.".into());
        }
        Ok(vec![Effect::Send { service: 3, data: json!({ "ver": "1", "cmd": 4 }) }])
    }

    pub fn refresh_apps(&self) -> Vec<Effect> {
        vec![Effect::Send { service: 17, data: json!({ "ver": "1", "cmd": 2 }) }]
    }

    pub fn stop_pkg(&mut self, pkg: &str) -> Vec<Effect> {
        let id = self
            .sessions
            .iter()
            .find(|(_, session)| session.pkg == pkg && !session.quiet)
            .map(|(id, _)| id.clone())
            .unwrap_or_default();
        if !id.is_empty() {
            return self.stop_session(&id);
        }
        if self.starting && self.opened_at.as_ref().is_some_and(|(name, _)| name == pkg) {
            return self.cancel_open();
        }
        if pkg == "webcam" && self.webcam_armed {
            self.webcam_armed = false;
            return vec![
                Effect::Send { service: 3, data: json!({ "ver": "1", "cmd": 2 }) },
                Effect::Note("Closed the phone camera.".into()),
                Effect::Obs { on: false },
                Effect::Publish,
            ];
        }
        Vec::new()
    }

    pub fn stop_session(&mut self, session_id: &str) -> Vec<Effect> {
        let id = if session_id.is_empty() { self.visible.clone().unwrap_or_default() } else { session_id.to_string() };
        if id.is_empty() || !self.sessions.contains_key(&id) {
            if self.starting {
                return self.cancel_open();
            }
            return Vec::new();
        }
        let quiet = self.sessions.get(&id).is_some_and(|session| session.quiet);
        let webcam = self.sessions.get(&id).is_some_and(|session| session.pkg == "webcam");
        if webcam {
            self.webcam_armed = false;
        }
        self.suppress_close.insert(id.clone(), true);
        self.sessions.remove(&id);
        if self.visible.as_deref() == Some(&id) {
            self.visible = self.sessions.iter().find(|(_, session)| session.pkg == "mirror").map(|(key, _)| key.clone());
        }
        self.starting = false;
        let stop = if webcam {
            json!({ "ver": "1", "cmd": 2 })
        } else {
            json!({ "ver": "1", "cmd": 7, "data": { "sessionId": id.clone(), "error": 0 } })
        };
        let mut effects = vec![
            Effect::Send { service: if webcam { 3 } else { 15 }, data: stop },
            Effect::Closed { session_id: id },
            Effect::Publish,
        ];
        if !quiet {
            effects.push(Effect::Note(if webcam { "Closed the phone camera.".into() } else { "Closed the phone screen.".into() }));
        }
        if webcam {
            effects.push(Effect::Obs { on: false });
        }
        effects
    }

    pub fn answer(&mut self, session_id: &str, sdp: &str) -> Result<Vec<Effect>, String> {
        if sdp.len() > 100_000 || !sdp.contains("v=") {
            return Err("The screen answer was not usable.".into());
        }
        let Some(session) = self.sessions.get(session_id) else {
            return Err("The phone closed that screen.".into());
        };
        let user_id = session.user_id;
        let web = serde_json::to_string(&json!({ "type": "answer", "sdp": sdp })).map_err(|_| "Could not write the screen answer.")?;
        let quiet = session.quiet;
        let webcam = session.pkg == "webcam";
        self.starting = false;
        if webcam {
            return Ok(vec![
                Effect::Send {
                    service: 3,
                    data: json!({
                        "ver": "1",
                        "cmd": 17,
                        "data": { "sessionId": wire_session_id(session_id), "type": "answer", "webRTCData": web }
                    }),
                },
                Effect::Send { service: 3, data: json!({ "ver": "1", "cmd": 13 }) },
                Effect::Note("Answered the phone camera.".into()),
            ]);
        }
        // The phone reads service 15 fields from `data`, the same place peer start lives.
        // A flat answer is ignored, so the phone never sends a screen address and stops the session.
        let mut effects = vec![
            Effect::Send {
                service: 15,
                data: json!({
                    "ver": "1",
                    "cmd": 4,
                    "data": {
                        "sessionId": session_id,
                        "error": 0,
                        "isWebRTC": 1,
                        "resize": 0,
                        "userId": user_id,
                        "webRTCData": web
                    }
                }),
            },
            Effect::Send { service: 15, data: json!({ "ver": "1", "cmd": 9 }) },
        ];
        if !quiet {
            effects.push(Effect::Note(answer_shape(sdp)));
        }
        Ok(effects)
    }

    pub fn ice(&self, session_id: &str, candidate: &str, label: i64, mid: &str) -> Option<Effect> {
        let session = self.sessions.get(session_id)?;
        if candidate.is_empty() || candidate.len() > 2000 {
            return None;
        }
        let inner = serde_json::to_string(&json!({
            "type": "candidate",
            "label": label,
            "id": mid,
            "candidate": candidate
        }))
        .ok()?;
        if session.pkg == "webcam" {
            return Some(Effect::Send {
                service: 3,
                data: json!({
                    "ver": "1",
                    "cmd": 18,
                    "data": { "sessionId": wire_session_id(session_id), "type": "candidate", "webRTCData": inner }
                }),
            });
        }
        Some(Effect::Send {
            service: 15,
            data: json!({
                "ver": "1",
                "cmd": 20,
                "data": {
                    "sessionId": session_id,
                    "type": "candidate",
                    "pkg": session.pkg,
                    "userId": session.user_id,
                    "data": inner
                }
            }),
        })
    }

    pub fn cross_start(&mut self) -> Vec<Effect> {
        self.cross_cid = self.next_cid;
        self.next_cid = self.next_cid.saturating_add(1);
        vec![
            Effect::Send {
                service: 26,
                data: json!({
                    "ver": "1",
                    "cmd": 1,
                    "cid": self.cross_cid,
                    "code": 0,
                    "trigger": 1,
                    "controller": true
                }),
            },
            Effect::Note("Asking to control the phone.".into()),
        ]
    }

    pub fn cross_stop(&mut self) -> Vec<Effect> {
        if !self.cross_active && self.cross_cid == 0 {
            return vec![Effect::Note("Cross control is not on.".into())];
        }
        let cid = self.cross_cid;
        self.cross_active = false;
        vec![
            Effect::Send { service: 26, data: json!({ "ver": "1", "cmd": 4, "show": false }) },
            Effect::Send { service: 26, data: json!({ "ver": "1", "cmd": 5, "cid": cid }) },
            Effect::Status,
            Effect::Note("Stopped controlling the phone.".into()),
            Effect::Publish,
        ]
    }

    pub fn cross_show(&self, show: bool) -> Option<Effect> {
        if !self.cross_active {
            return None;
        }
        Some(Effect::Send { service: 26, data: json!({ "ver": "1", "cmd": 4, "show": show }) })
    }

    pub fn input(&mut self, display_id: i64, data: Value) -> Vec<Effect> {
        let mut effects = Vec::new();
        if display_id >= 0 && !self.caps_sent.contains_key(&display_id) {
            self.caps_sent.insert(display_id, true);
            effects.push(Effect::Send {
                service: 16,
                data: json!({
                    "cmd": 1,
                    "data": { "ver": 1, "displayId": display_id, "caps": 3 },
                    "messageId": 0,
                    "instanceId": 0
                }),
            });
        }
        effects.push(Effect::Send {
            service: 16,
            data: json!({ "cmd": data["cmd"], "data": data["data"], "messageId": 0, "instanceId": 0 }),
        });
        effects
    }

    pub fn store_icons(&mut self, icons: HashMap<String, String>) {
        if icons.is_empty() {
            return;
        }
        self.icons.extend(icons);
        self.icons_revision = self.icons_revision.saturating_add(1);
    }

    pub fn store_files(&mut self, files: Vec<FileView>) {
        for file in files {
            self.files.push_front(file);
        }
        while self.files.len() > FILE_LIMIT {
            self.files.pop_back();
        }
    }

    pub fn known_file(&self, path: &str) -> bool {
        self.files.iter().any(|file| file.path == path)
    }

    /// Drop a start the phone never answered. A newer start has a different token.
    pub fn give_up(&mut self, token: u64) -> bool {
        if self.starting && self.start_token == token {
            self.starting = false;
            true
        } else {
            false
        }
    }

    fn begin_start(&mut self) -> u64 {
        self.starting = true;
        self.start_at = Instant::now();
        self.start_token = self.start_token.wrapping_add(1);
        self.start_token
    }

    /// A second tap while the phone is still opening used to lock the button.
    /// A fresh start is left alone. An old one is cancelled so the tap can try again.
    fn gate_start(&mut self) -> StartGate {
        if !self.starting {
            return StartGate::Go(Vec::new());
        }
        if self.start_at.elapsed() < Duration::from_secs(3) {
            return StartGate::Wait(vec![
                Effect::Note("Still waiting for the phone to open the screen.".into()),
                Effect::Publish,
            ]);
        }
        StartGate::Go(self.cancel_open())
    }

    fn remember_open(&mut self, pkg: &str) {
        self.opened_at = Some((pkg.to_string(), Instant::now()));
    }

    fn open_is_fresh(&self, pkg: &str) -> bool {
        self.opened_at.as_ref().is_some_and(|(name, at)| name == pkg && at.elapsed() < Duration::from_secs(20))
    }

    fn cancel_open(&mut self) -> Vec<Effect> {
        self.starting = false;
        vec![
            Effect::Send { service: 1, data: json!({ "ver": "1.0.0", "cmd": 5 }) },
            Effect::Note("Stopped the screen the phone was opening.".into()),
            Effect::Publish,
        ]
    }

    fn webcam_id(&self) -> Option<String> {
        self.sessions.iter().find(|(_, session)| session.pkg == "webcam").map(|(id, _)| id.clone())
    }

    fn on_webcam(&mut self, raw: &Value) -> Vec<Effect> {
        let cmd = json_i64_opt(raw.get("cmd")).unwrap_or(-1);
        let nested = match raw.get("data") {
            Some(Value::String(text)) => serde_json::from_str(text).unwrap_or(Value::Null),
            Some(other) => other.clone(),
            None => Value::Null,
        };
        match cmd {
            0 => match json_i64_opt(nested.get("isReply")) {
                Some(0) => {
                    self.webcam_armed = true;
                    vec![
                        Effect::Send { service: 3, data: webcam_accept() },
                        Effect::Note("The phone asked to share its camera.".into()),
                        Effect::Publish,
                    ]
                }
                Some(1) => {
                    self.webcam_armed = true;
                    vec![
                        Effect::Send { service: 3, data: webcam_go() },
                        Effect::Note("The phone accepted the camera.".into()),
                        Effect::Publish,
                    ]
                }
                _ => Vec::new(),
            },
            1 => self.webcam_offer(&nested),
            2 => self.webcam_gone(),
            18 => self.webcam_ice(&nested),
            65535 => {
                let code = json_i64_opt(nested.get("code")).unwrap_or(0);
                let mut effects = self.webcam_gone();
                effects.push(Effect::Note(webcam_error(code)));
                effects
            }
            _ => Vec::new(),
        }
    }

    fn webcam_offer(&mut self, nested: &Value) -> Vec<Effect> {
        let Some(value) = webrtc_value(nested) else {
            return vec![Effect::Note("The phone camera description could not be read.".into())];
        };
        let Some(sdp) = sdp_of_value(&value, "offer") else {
            return vec![Effect::Note("The phone camera description could not be read.".into())];
        };
        // Leave the offer intact. The camera window plays it. Filtering it to
        // H.264 makes that window reject the video line.
        // The phone parses sessionId with Gson as a long. The word "webcam" crashes that app.
        let session_id = webcam_session_id(nested, self.webcam_id());
        self.sessions.insert(
            session_id.clone(),
            Session {
                pkg: "webcam".into(),
                title: "Phone camera".into(),
                user_id: 0,
                display_id: -1,
                width: 1280,
                height: 720,
                quiet: false,
                saw_offer: true,
                support_touch: false,
            },
        );
        self.starting = false;
        vec![
            Effect::Offer { session_id, sdp, quiet: false, pkg: "webcam".into() },
            Effect::Note("The phone offered its camera.".into()),
            Effect::Publish,
        ]
    }

    fn webcam_ice(&mut self, nested: &Value) -> Vec<Effect> {
        let Some(session_id) = self.webcam_id() else { return Vec::new() };
        let Some(inner) = webrtc_value(nested).or_else(|| ice_value(nested)) else { return Vec::new() };
        let Some(candidate) = inner.get("candidate").and_then(Value::as_str).map(str::trim).filter(|text| text.contains("candidate:")) else {
            return Vec::new();
        };
        let candidate = candidate.strip_prefix("a=").unwrap_or(candidate).to_string();
        let label = json_i64(inner.get("label").or_else(|| inner.get("sdpMLineIndex")));
        let mid = match inner.get("id").or_else(|| inner.get("mid")) {
            Some(Value::String(text)) if !text.is_empty() => text.clone(),
            Some(Value::Number(number)) => number.to_string(),
            _ => "0".to_string(),
        };
        vec![Effect::Ice { session_id, candidate, label, mid }]
    }

    fn webcam_gone(&mut self) -> Vec<Effect> {
        self.webcam_armed = false;
        let Some(session_id) = self.webcam_id() else {
            return vec![Effect::Obs { on: false }, Effect::Publish];
        };
        self.sessions.remove(&session_id);
        vec![Effect::Closed { session_id }, Effect::Obs { on: false }, Effect::Publish]
    }

    fn on_view(&mut self, data: &Value) -> Vec<Effect> {
        let cmd = data.get("cmd").and_then(Value::as_i64).unwrap_or(-1);
        match cmd {
            0 => {
                let view = data.get("view").and_then(Value::as_i64).unwrap_or(1);
                let label = match view {
                    2 => "Phone desktop",
                    3 => "Phone",
                    _ => "Phone screen",
                };
                let mut effects = vec![
                    Effect::Send {
                        service: 1,
                        data: json!({ "ver": "1.0.0", "cmd": 1, "view": view, "reply": 1, "drawerMode": false }),
                    },
                    Effect::Note(format!("The phone asked to open {label}.")),
                    Effect::Publish,
                ];
                // A start this Mac already asked for keeps its timer. Resetting it
                // made every later tap say the phone was already opening a screen.
                if !self.starting {
                    let token = self.begin_start();
                    effects.push(Effect::WatchStart { token });
                }
                effects
            }
            11 => {
                self.starting = false;
                let code = data.get("errorCode").and_then(Value::as_i64).unwrap_or(0);
                vec![Effect::Note(view_error(code).into()), Effect::Publish]
            }
            _ => Vec::new(),
        }
    }

    fn on_fex(&mut self, data: &Value) -> Vec<Effect> {
        let Some(port) = port_of(data) else { return Vec::new() };
        let Some(fingerprint) = text_field(data, "fingerprint") else { return Vec::new() };
        let token = text_field(data, "token").unwrap_or_default();
        vec![
            Effect::Pull(PullJob { port, fingerprint, token, icons: false }),
            Effect::Note("Receiving a file from the phone.".into()),
        ]
    }

    fn on_notification(&mut self, service: i64, data: &Value) -> Vec<Effect> {
        let key = text_field(data, "key").unwrap_or_default();
        if key.is_empty() {
            return Vec::new();
        }
        let event = data.get("event").and_then(Value::as_i64).unwrap_or(0);
        if event == 2 {
            let removed = self.notifications.iter().position(|(id, _)| id == &key);
            if let Some(index) = removed {
                self.notifications.remove(index);
                return vec![Effect::Publish];
            }
            return Vec::new();
        }
        let body = data.get("data").cloned().unwrap_or(Value::Null);
        let app = text_field(&body, "apk_name").unwrap_or_else(|| "Phone".into());
        let pkg = text_field(&body, "package_name").unwrap_or_default();
        let title = text_field(&body, "title").filter(|text| !text.is_empty()).unwrap_or_else(|| app.clone());
        let content = text_field(&body, "content").unwrap_or_default();
        let mut actions = Vec::new();
        let mut can_reply = false;
        if let Some(list) = body_actions(data) {
            for (index, action) in list.iter().enumerate() {
                let name = text_field(action, "action_name").unwrap_or_default();
                if name.is_empty() {
                    continue;
                }
                let id = action.get("action_id").and_then(Value::as_i64).unwrap_or(index as i64);
                let remote = action.get("remote_input").and_then(Value::as_bool).unwrap_or(false);
                if remote {
                    can_reply = true;
                }
                actions.push(ActionView { id, name });
            }
        }
        let fingerprint = format!("{title}\n{content}\n{}", actions.len());
        let previous = self.notifications.iter().find(|(id, _)| id == &key).map(|(_, notice)| notice.clone());
        if let Some(previous) = &previous {
            if previous.fingerprint == fingerprint && previous.shown_at.elapsed() < Duration::from_secs(1) {
                return Vec::new();
            }
        }
        self.notifications.retain(|(id, _)| id != &key);
        self.notifications.push_front((
            key,
            Notice {
                service,
                app: app.clone(),
                pkg,
                title: title.clone(),
                content: content.clone(),
                can_reply,
                actions,
                fingerprint: fingerprint.clone(),
                shown_at: Instant::now(),
            },
        ));
        while self.notifications.len() > NOTICE_LIMIT {
            self.notifications.pop_back();
        }
        let mut effects = vec![Effect::Publish];
        if previous.as_ref().is_none_or(|item| item.fingerprint != fingerprint) {
            effects.push(Effect::Banner {
                title: clip_banner(&title),
                body: clip_banner(&content),
            });
        }
        effects
    }

    fn on_clipboard(&mut self, data: &Value) -> Vec<Effect> {
        let msg = data.get("msgId").and_then(Value::as_i64).unwrap_or(-1);
        let nested = data.get("data").cloned().unwrap_or(Value::Null);
        match msg {
            1 => {
                self.clipboard_ready = true;
                vec![
                    Effect::Send { service: 9, data: json!({ "msgId": 2, "data": { "version": 1 } }) },
                    Effect::Note("Clipboard sync is on.".into()),
                    Effect::Publish,
                ]
            }
            3 => {
                let clip_id = nested.get("clipId").and_then(Value::as_i64).unwrap_or(0);
                if clip_id == 0 {
                    return Vec::new();
                }
                if let Some(text) = nested.get("text").and_then(Value::as_str) {
                    if text.is_empty() || self.last_clip.as_deref() == Some(text) {
                        return Vec::new();
                    }
                    self.last_clip = Some(text.to_string());
                    return vec![Effect::Paste(text.to_string()), Effect::Note("Copied from the phone.".into())];
                }
                let files = nested.get("filelist").or_else(|| nested.get("fileList")).and_then(Value::as_array);
                if files.is_some() {
                    return vec![
                        Effect::Send { service: 9, data: json!({ "msgId": 4, "data": { "clipId": clip_id } }) },
                        Effect::Note("The phone copied a file. Saving it.".into()),
                    ];
                }
                Vec::new()
            }
            7 => {
                self.clipboard_ready = false;
                vec![Effect::Note("The phone turned clipboard sync off.".into()), Effect::Publish]
            }
            _ => Vec::new(),
        }
    }

    fn on_stream(&mut self, data: &Value) -> Vec<Effect> {
        let cmd = data.get("cmd").and_then(Value::as_i64).unwrap_or(-1);
        match cmd {
            2 => self.accept_invite(data),
            3 => self.on_offer(data),
            8 => self.on_state(data),
            12 => {
                let pkg = nested_text(data, "pkg").unwrap_or_default();
                if let Some((id, _)) = self.sessions.iter().find(|(_, session)| session.pkg == pkg) {
                    self.visible = Some(id.clone());
                }
                self.starting = false;
                vec![Effect::Note("The phone opened that app on a screen that is already up.".into()), Effect::Publish]
            }
            13 => self.on_focus(data),
            16 => self.on_rotation(data),
            20 => self.on_ice(data),
            _ => Vec::new(),
        }
    }

    fn accept_invite(&mut self, data: &Value) -> Vec<Effect> {
        let nested = data.get("data").cloned().unwrap_or_else(|| data.clone());
        let pkg = text_field(&nested, "pkg").unwrap_or_else(|| "mirror".into());
        if self.sessions.values().any(|session| session.pkg == pkg) {
            return Vec::new();
        }
        let cmp = text_field(&nested, "cmp").unwrap_or_default();
        let user_id = nested.get("userId").and_then(Value::as_i64).unwrap_or(0);
        let source = nested.get("source").and_then(Value::as_i64).unwrap_or(1);
        let title = text_field(&nested, "pkgName").filter(|text| !text.is_empty()).unwrap_or_else(|| view_title(&pkg));
        if self.pending_quiet.contains_key(&pkg) {
            return Vec::new();
        }
        // Show phone screen already sent this start. A second copy makes the phone drop it.
        if self.open_is_fresh(&pkg) {
            let quiet = self
                .awaiting
                .as_ref()
                .is_some_and(|wait| wait.quiet && wait.pkg == pkg && wait.until > Instant::now());
            self.awaiting = None;
            if quiet {
                return Vec::new();
            }
            return vec![Effect::Note("The phone is preparing the screen.".into())];
        }
        let quiet = self
            .awaiting
            .as_ref()
            .is_some_and(|wait| wait.quiet && wait.pkg == pkg && wait.until > Instant::now());
        self.awaiting = None;
        self.pending_quiet.insert(pkg.clone(), quiet);
        self.pending_title.insert(pkg.clone(), title);
        let token = self.begin_start();
        let mut effects = vec![
            Effect::Send { service: 15, data: stream_start(&pkg, &cmp, user_id, source, self.instance_id) },
            Effect::WatchStart { token },
        ];
        if !quiet {
            effects.push(Effect::Note("The phone is preparing the screen.".into()));
        }
        effects
    }

    fn on_offer(&mut self, data: &Value) -> Vec<Effect> {
        let nested = offer_fields(data);
        let Some(session_id) = id_string(nested.get("sessionId")) else { return Vec::new() };
        let mut pkg = text_field(&nested, "pkg").unwrap_or_default();
        if pkg.is_empty() {
            if let Some((name, at)) = &self.opened_at {
                if at.elapsed() < Duration::from_secs(20) {
                    pkg = name.clone();
                }
            }
        }
        if pkg.is_empty() {
            pkg = "mirror".into();
        }
        let user_id = nested.get("userId").and_then(Value::as_i64).unwrap_or(0);
        let width = nested.get("width").and_then(Value::as_i64).unwrap_or(0);
        let height = nested.get("height").and_then(Value::as_i64).unwrap_or(0);
        let display_id = nested.get("displayId").and_then(Value::as_i64).unwrap_or(-1);
        let title = self
            .sessions
            .get(&session_id)
            .map(|session| session.title.clone())
            .or_else(|| self.pending_title.get(&pkg).cloned())
            .or_else(|| self.apps.iter().find(|app| app.pkg == pkg).map(|app| app.name.clone()))
            .unwrap_or_else(|| view_title(&pkg));
        let quiet = self.sessions.get(&session_id).is_some_and(|session| session.quiet)
            || self.pending_quiet.get(&pkg).copied().unwrap_or(false);
        self.pending_quiet.remove(&pkg);
        self.pending_title.remove(&pkg);
        let session = self.sessions.entry(session_id.clone()).or_insert(Session {
            pkg,
            title,
            user_id,
            display_id,
            width,
            height,
            quiet,
            saw_offer: false,
            support_touch: false,
        });
        session.user_id = user_id;
        if width > 0 {
            session.width = width;
        }
        if height > 0 {
            session.height = height;
        }
        if display_id >= 0 {
            session.display_id = display_id;
        }
        let quiet_session = session.quiet;
        let on_desk = session.pkg == "mirror";
        let repeat = session.saw_offer;
        if !quiet_session && on_desk {
            self.visible = Some(session_id.clone());
        }
        self.starting = false;
        let mut effects = Vec::new();
        match webrtc_value(&nested).as_ref().and_then(|value| sdp_of_value(value, "offer")) {
            // A later cmd 3 only refreshes the display size. Answering it again
            // tears down the picture, and the window keeps the black frame it already painted.
            Some(sdp) if !repeat => {
                if let Some(session) = self.sessions.get_mut(&session_id) {
                    session.saw_offer = true;
                }
                let names = video_codec_names(&sdp);
                let pkg_name = self.sessions.get(&session_id).map(|session| session.pkg.clone()).unwrap_or_else(|| "mirror".into());
                // Leave the offer intact. Filtering it to H.264 makes this Mac's
                // web view reject the video section. The phone then encodes a codec
                // the view can play, and the view decodes it in hardware.
                effects.push(Effect::Offer { session_id, sdp, quiet: quiet_session, pkg: pkg_name });
                if !quiet_session {
                    let note = if names.is_empty() {
                        "The phone offered its screen.".to_string()
                    } else {
                        format!("The phone offered its screen ({}).", names.join(", "))
                    };
                    effects.push(Effect::Note(note));
                }
            }
            Some(_) => {}
            None if nested.get("webRTCData").is_some() => {
                effects.push(Effect::Note("The phone's screen description could not be read.".into()));
            }
            None => {}
        }
        effects.extend(self.arm_cap(display_id));
        effects.push(Effect::Publish);
        effects
    }

    fn on_state(&mut self, data: &Value) -> Vec<Effect> {
        let nested = offer_fields(data);
        let state = nested.get("state").and_then(Value::as_i64).unwrap_or(-1);
        let session_id = id_string(nested.get("sessionId")).unwrap_or_default();
        let display_id = nested.get("displayId").and_then(Value::as_i64).unwrap_or(-1);
        if state == 1 {
            let touch = json_flag(nested.get("supportTouch"));
            let mut announce_touch = false;
            if let Some(session) = self.sessions.get_mut(&session_id) {
                if display_id >= 0 {
                    session.display_id = display_id;
                }
                if touch && !session.support_touch && !is_view_pkg(&session.pkg) {
                    announce_touch = true;
                }
                session.support_touch |= touch;
            }
            let quiet = self.sessions.get(&session_id).is_some_and(|session| session.quiet);
            let mut effects = self.arm_cap(display_id);
            if !quiet {
                effects.push(Effect::Note("The phone started sending its screen.".into()));
                if announce_touch {
                    effects.push(Effect::Note("This app screen takes a finger, so a hold and a swipe can land.".into()));
                }
            }
            effects.push(Effect::Publish);
            return effects;
        }
        if state == 0 {
            self.starting = false;
            let reason = nested.get("reason").and_then(Value::as_i64).unwrap_or(0);
            let suppressed = self.suppress_close.remove(&session_id).is_some();
            self.sessions.remove(&session_id);
            if self.visible.as_deref() == Some(&session_id) {
                self.visible = self.sessions.keys().next().cloned();
            }
            let mut effects = vec![Effect::Closed { session_id }, Effect::Publish];
            if !suppressed {
                let text = if reason != 0 {
                    stream_reason(reason).to_string()
                } else {
                    "The phone stopped the screen.".to_string()
                };
                effects.push(Effect::Note(text));
            }
            return effects;
        }
        Vec::new()
    }

    fn on_focus(&mut self, data: &Value) -> Vec<Effect> {
        let nested = offer_fields(data);
        let display_id = nested.get("displayId").and_then(Value::as_i64).unwrap_or(-1);
        let title = text_field(&nested, "appName").unwrap_or_default();
        let pkg = text_field(&nested, "pkg").unwrap_or_default();
        let mut quiet_id = None;
        for (id, session) in &mut self.sessions {
            let matches = (display_id >= 0 && session.display_id == display_id) || (session.display_id < 0 && !pkg.is_empty() && session.pkg == pkg);
            if matches {
                if !title.is_empty() {
                    session.title = title.clone();
                }
                if display_id >= 0 {
                    session.display_id = display_id;
                }
                if session.quiet {
                    quiet_id = Some(id.clone());
                } else {
                    self.visible = Some(id.clone());
                }
            }
        }
        let mut effects = vec![Effect::Publish];
        if let Some(session_id) = quiet_id {
            effects.push(Effect::CloseLater { session_id, secs: 3 });
        }
        effects
    }

    fn on_rotation(&mut self, data: &Value) -> Vec<Effect> {
        let nested = offer_fields(data);
        let Some(session_id) = id_string(nested.get("sessionId")) else { return Vec::new() };
        if let Some(session) = self.sessions.get_mut(&session_id) {
            if let Some(display_id) = nested.get("displayId").and_then(Value::as_i64) {
                session.display_id = display_id;
            }
        }
        vec![Effect::Publish]
    }

    fn on_ice(&mut self, data: &Value) -> Vec<Effect> {
        let nested = offer_fields(data);
        let session_id = id_string(nested.get("sessionId")).or_else(|| id_string(data.get("sessionId")));
        let Some(session_id) = session_id else {
            return self.note_ice_shape();
        };
        let Some(inner) = ice_value(&nested).or_else(|| ice_value(data)) else {
            return self.note_ice_shape();
        };
        let Some(candidate) = inner.get("candidate").and_then(Value::as_str).map(str::trim).filter(|text| text.contains("candidate:")) else {
            return self.note_ice_shape();
        };
        let candidate = candidate.strip_prefix("a=").unwrap_or(candidate).to_string();
        let label = json_i64(inner.get("label").or_else(|| inner.get("sdpMLineIndex")));
        let mid = match inner.get("id").or_else(|| inner.get("mid")) {
            Some(Value::String(text)) if !text.is_empty() => text.clone(),
            Some(Value::Number(number)) => number.to_string(),
            _ => "0".to_string(),
        };
        let mut effects = vec![Effect::Ice { session_id, candidate, label, mid }];
        if !self.ice_seen_noted {
            self.ice_seen_noted = true;
            effects.push(Effect::Note("The phone sent a screen address.".into()));
        }
        effects
    }

    fn note_ice_shape(&mut self) -> Vec<Effect> {
        if self.ice_shape_noted {
            return Vec::new();
        }
        self.ice_shape_noted = true;
        vec![Effect::Note("The phone sent a screen address this Mac could not read.".into())]
    }

    fn on_apps(&mut self, data: &Value) -> Vec<Effect> {
        let cmd = data.get("cmd").and_then(Value::as_i64).unwrap_or(-1);
        if cmd == 10 {
            return vec![Effect::Publish];
        }
        if cmd != 1 {
            return Vec::new();
        }
        self.apps = read_apps(data.get("apps")).into_iter().chain(read_apps(data.get("wpApps"))).collect();
        let mut effects = vec![Effect::Note(format!("{} apps from the phone.", self.apps.len())), Effect::Publish];
        if let (Some(port), Some(fingerprint)) = (port_of(data), text_field(data, "fingerprint")) {
            effects.push(Effect::Pull(PullJob {
                port,
                fingerprint,
                token: text_field(data, "token").unwrap_or_default(),
                icons: true,
            }));
        }
        effects
    }

    fn on_cross(&mut self, data: &Value) -> Vec<Effect> {
        let cmd = data.get("cmd").and_then(Value::as_i64).unwrap_or(-1);
        match cmd {
            0 => self.cross_start(),
            2 => {
                let code = data.get("code").and_then(Value::as_i64).unwrap_or(0);
                if code == 1 {
                    self.cross_active = true;
                    self.cross_width = data.get("width").and_then(Value::as_i64).unwrap_or(0);
                    self.cross_height = data.get("height").and_then(Value::as_i64).unwrap_or(0);
                    if let Some(cid) = data.get("cid").and_then(Value::as_i64) {
                        self.cross_cid = cid;
                    }
                    let mut effects = self.arm_cap(0);
                    effects.push(Effect::Status);
                    effects.push(Effect::Note("Controlling the phone.".into()));
                    effects.push(Effect::Publish);
                    effects
                } else {
                    self.cross_active = false;
                    vec![
                        Effect::Note("The phone declined cross control. It may need this Mac allowed, or another computer is already controlling it.".into()),
                        Effect::Publish,
                    ]
                }
            }
            5 => {
                let cid = data.get("cid").and_then(Value::as_i64).unwrap_or(self.cross_cid);
                self.cross_active = false;
                vec![
                    Effect::Send { service: 26, data: json!({ "ver": "1", "cmd": 6, "cid": cid }) },
                    Effect::Status,
                    Effect::Note("The phone ended cross control.".into()),
                    Effect::Publish,
                ]
            }
            7 => {
                self.cross_width = data.get("width").and_then(Value::as_i64).unwrap_or(self.cross_width);
                self.cross_height = data.get("height").and_then(Value::as_i64).unwrap_or(self.cross_height);
                vec![Effect::Publish]
            }
            _ => Vec::new(),
        }
    }

    fn on_station(&mut self, data: &Value) -> Vec<Effect> {
        let Some(body) = station_body(data) else { return Vec::new() };
        let type_id = body.get("type").and_then(Value::as_i64);
        if type_id == Some(1) {
            if let Some(share) = phone_share_from(&body) {
                self.phone_share = Some(share);
                self.share_gen = self.share_gen.wrapping_add(1);
                return vec![
                    Effect::Note("The phone opened its folders.".into()),
                    Effect::ShareReady,
                    Effect::Publish,
                ];
            }
            return Vec::new();
        }
        if type_id == Some(0) && !self.station_noted {
            self.station_noted = true;
            return vec![Effect::Note("The phone asked to open a folder on this Mac. Sharing a Mac folder is not set up. Phone folders open from Files.".into())];
        }
        Vec::new()
    }

    fn arm_cap(&mut self, display_id: i64) -> Vec<Effect> {
        if display_id < 0 || self.caps_sent.contains_key(&display_id) {
            return Vec::new();
        }
        self.caps_sent.insert(display_id, true);
        vec![Effect::Send {
            service: 16,
            data: json!({
                "cmd": 1,
                "data": { "ver": 1, "displayId": display_id, "caps": 3 },
                "messageId": 0,
                "instanceId": 0
            }),
        }]
    }
}

/// Screen share from this Mac. `cmp` and `shortcutId` stay out: the phone treats a PC view start that way.
fn view_start(pkg: &str, instance_id: i64) -> Value {
    let mut value = peer_start(pkg, "", 0, 0, instance_id);
    if let Some(map) = value.get_mut("data").and_then(Value::as_object_mut) {
        map.remove("cmp");
        map.remove("shortcutId");
    }
    value
}

fn is_view_pkg(pkg: &str) -> bool {
    matches!(pkg, "mirror" | "desktop" | "mobile" | "webcam")
}

/// An app uses the phone's own screen. A wide virtual display stays black for some apps.
fn stream_start(pkg: &str, cmp: &str, user_id: i64, source: i64, instance_id: i64) -> Value {
    let mut value = peer_start(pkg, cmp, user_id, source, instance_id);
    if is_view_pkg(pkg) {
        return value;
    }
    if let Some(map) = value.get_mut("data").and_then(Value::as_object_mut) {
        map.insert("supportCustomDisplay".into(), json!(false));
    }
    value
}

pub fn peer_start(pkg: &str, cmp: &str, user_id: i64, source: i64, instance_id: i64) -> Value {
    json!({
        "ver": "1",
        "cmd": 1,
        "data": {
            "pkg": pkg,
            "cmp": cmp,
            "shortcutId": Value::Null,
            "userId": user_id,
            "source": source,
            "width": 1920,
            "height": 1080,
            "phyWidth": 344,
            "phyHeight": 194,
            "displayWidth": 1920,
            "displayHeight": 1080,
            "displayDensity": 160,
            "displayScale": 1.0,
            "align": 0,
            "instanceId": instance_id,
            "port": 0,
            "supportedMediaFormat": 1,
            "supportWebRTC": 1,
            "supportCustomDisplay": false,
            "vrSupportUnfolded": false
        }
    })
}

pub fn decode_icon(name: &str, bytes: &[u8]) -> Option<(String, String)> {
    if bytes.len() < 8 || bytes.len() > 256 * 1024 || &bytes[..8] != b"\x89PNG\r\n\x1a\n" {
        return None;
    }
    let clean = std::path::Path::new(name).file_name()?.to_str()?;
    let encoded = base64::engine::general_purpose::STANDARD.encode(bytes);
    Some((clean.to_string(), format!("data:image/png;base64,{encoded}")))
}

fn clip_text(text: &str) -> String {
    if text.chars().count() <= 100_000 {
        return text.to_string();
    }
    text.chars().take(100_000).collect()
}

fn clip_banner(text: &str) -> String {
    let flat: String = text.chars().map(|ch| if ch.is_control() { ' ' } else { ch }).take(180).collect();
    flat.trim().to_string()
}

fn text_field(value: &Value, key: &str) -> Option<String> {
    value.get(key).and_then(Value::as_str).map(str::trim).filter(|text| !text.is_empty()).map(str::to_string)
}

fn nested_text(value: &Value, key: &str) -> Option<String> {
    text_field(value, key).or_else(|| value.get("data").and_then(|nested| text_field(nested, key)))
}

fn port_of(value: &Value) -> Option<u16> {
    let port = value.get("port").and_then(Value::as_i64)?;
    u16::try_from(port).ok().filter(|port| *port != 0)
}

fn body_actions(data: &Value) -> Option<&Vec<Value>> {
    data.get("actions").and_then(Value::as_array)
}

fn offer_fields(data: &Value) -> Value {
    if data.get("sessionId").is_some() || data.get("state").is_some() || data.get("webRTCData").is_some() {
        return data.clone();
    }
    data.get("data").cloned().unwrap_or_else(|| data.clone())
}

fn id_string(value: Option<&Value>) -> Option<String> {
    match value? {
        Value::String(text) if !text.is_empty() => Some(text.clone()),
        Value::Number(number) => Some(number.to_string()),
        _ => None,
    }
}

fn webrtc_value(data: &Value) -> Option<Value> {
    match data.get("webRTCData")? {
        Value::String(text) => serde_json::from_str(text).ok(),
        Value::Object(_) => Some(data["webRTCData"].clone()),
        _ => None,
    }
}

fn ice_value(data: &Value) -> Option<Value> {
    if let Some(text) = data.get("candidate").and_then(Value::as_str) {
        if text.contains("candidate:") {
            return Some(json!({ "candidate": text, "label": data.get("label").cloned().unwrap_or(Value::from(0)), "id": data.get("id").cloned().unwrap_or(Value::from("0")) }));
        }
    }
    match data.get("data")? {
        Value::String(text) => {
            let text = text.trim();
            if text.starts_with('{') {
                serde_json::from_str(text).ok()
            } else if text.contains("candidate:") {
                Some(json!({
                    "candidate": text,
                    "label": data.get("label").cloned().unwrap_or(Value::from(0)),
                    "id": data.get("id").cloned().unwrap_or(Value::from("0"))
                }))
            } else {
                None
            }
        }
        Value::Object(_) => Some(data["data"].clone()),
        _ => None,
    }
}

fn json_flag(value: Option<&Value>) -> bool {
    match value {
        Some(Value::Bool(flag)) => *flag,
        Some(Value::Number(number)) => number.as_i64() == Some(1),
        Some(Value::String(text)) => text == "true" || text == "1",
        _ => false,
    }
}

fn json_i64_opt(value: Option<&Value>) -> Option<i64> {
    match value? {
        Value::Number(number) => number.as_i64(),
        Value::String(text) => text.parse().ok(),
        Value::Bool(true) => Some(1),
        Value::Bool(false) => Some(0),
        _ => None,
    }
}

fn wire_session_id(session_id: &str) -> Value {
    session_id.parse::<i64>().map(Value::from).unwrap_or_else(|_| Value::from(1))
}

fn webcam_session_id(nested: &Value, existing: Option<String>) -> String {
    if let Some(id) = id_string(nested.get("sessionId")) {
        if id.parse::<i64>().is_ok() {
            return id;
        }
    }
    if let Some(id) = existing {
        if id.parse::<i64>().is_ok() {
            return id;
        }
    }
    SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_millis().to_string()).unwrap_or_else(|_| "1".into())
}

fn webcam_request() -> Value {
    json!({ "ver": "1", "cmd": 0, "data": { "minL": 1, "maxL": 1, "heartbeat": 1, "lite": 0 } })
}

fn webcam_accept() -> Value {
    json!({
        "ver": "1",
        "cmd": 0,
        "data": {
            "minL": 1,
            "maxL": 1,
            "heartbeat": 1,
            "isReply": 1,
            "port": 6466,
            "supportedMediaFormat": 1,
            "supportedWebRTC": 1
        }
    })
}

fn webcam_go() -> Value {
    json!({ "ver": "1", "cmd": 1, "data": { "port": 6466, "heartbeat": 1 } })
}

fn webcam_error(code: i64) -> String {
    let why = match code {
        1 => "the camera is in use on another computer",
        5 => "the phone's policy blocks the camera",
        6 | 7 => "the request was declined on the phone",
        8 => "this Android user cannot share the camera",
        9 => "the phone has no free video encoder",
        10 => "the camera is open in another app",
        _ => "the phone stopped the camera",
    };
    format!("The phone camera stopped ({code}): {why}.")
}

fn json_i64(value: Option<&Value>) -> i64 {
    match value {
        Some(Value::Number(number)) => number.as_i64().unwrap_or(0),
        Some(Value::String(text)) => text.parse().unwrap_or(0),
        _ => 0,
    }
}

fn sdp_of_value(value: &Value, kind: &str) -> Option<String> {
    let typed = value.get("type").and_then(Value::as_str);
    if typed.is_some() && typed != Some(kind) {
        return None;
    }
    value.get("sdp").and_then(Value::as_str).filter(|sdp| sdp.contains("v=")).map(str::to_string)
}

/// The phone lists H.265, VP8, VP9 and AV1 ahead of H.264. This phone's only H.264 line is
/// baseline level 5.1 (`420033`). Keep every H.264 payload, and the retransmission payload
/// that points at one of them. Leave a video section with no H.264 unchanged.
pub fn h264_only_sdp(sdp: &str) -> String {
    let normalized = sdp.replace("\r\n", "\n").replace('\r', "\n");
    let lines: Vec<String> = normalized.split('\n').filter(|line| !line.is_empty()).map(str::to_string).collect();
    if lines.is_empty() {
        return String::new();
    }
    let mut out = Vec::with_capacity(lines.len());
    let mut index = 0;
    while index < lines.len() {
        if !lines[index].starts_with("m=video ") {
            out.push(lines[index].clone());
            index += 1;
            continue;
        }
        let mut end = index + 1;
        while end < lines.len() && !lines[end].starts_with("m=") {
            end += 1;
        }
        let section = &lines[index..end];
        let mut keep = HashSet::new();
        for line in section {
            if let Some((pt, codec)) = rtpmap(line) {
                if codec.eq_ignore_ascii_case("H264") {
                    keep.insert(pt);
                }
            }
        }
        if keep.is_empty() {
            out.extend(section.iter().cloned());
            index = end;
            continue;
        }
        let h264 = keep.clone();
        for line in section {
            if let Some((pt, target)) = fmtp_apt(line) {
                if h264.contains(&target) {
                    keep.insert(pt);
                }
            }
        }
        let parts: Vec<&str> = section[0].split_whitespace().collect();
        if parts.len() < 3 {
            out.extend(section.iter().cloned());
            index = end;
            continue;
        }
        let pts: Vec<&str> = parts[3..].iter().copied().filter(|pt| keep.contains(*pt)).collect();
        if pts.is_empty() {
            out.extend(section.iter().cloned());
            index = end;
            continue;
        }
        let mut media = parts[..3].join(" ");
        for pt in pts {
            media.push(' ');
            media.push_str(pt);
        }
        out.push(media);
        for line in &section[1..] {
            if line.contains("urn:3gpp:video-orientation") {
                continue;
            }
            if let Some(pt) = payload_attribute(line) {
                if !keep.contains(&pt) {
                    continue;
                }
            }
            out.push(line.clone());
        }
        index = end;
    }
    let mut text = out.join("\r\n");
    text.push_str("\r\n");
    text
}

fn fmtp_params<'a>(line: &'a str, pt: &str) -> Option<&'a str> {
    let rest = line.strip_prefix("a=fmtp:")?;
    let (owner, params) = rest.split_once(' ')?;
    if owner == pt { Some(params) } else { None }
}

fn fmtp_value<'a>(params: &'a str, key: &str) -> Option<&'a str> {
    params.split(';').find_map(|part| {
        let (name, value) = part.trim().split_once('=')?;
        if name.trim().eq_ignore_ascii_case(key) { Some(value.trim()) } else { None }
    })
}

// One line for the activity log. It names the profile, DTLS setup, direction, and
// media-line count, and it does not include the SDP.
fn answer_shape(sdp: &str) -> String {
    let profile = answered_h264_profile(sdp).unwrap_or_else(|| "unknown".to_string());
    let mut setup = "setup unset";
    let mut direction = "direction unset";
    let mut lines = 0u32;
    let mut video = false;
    for line in sdp.split(['\r', '\n']) {
        let line = line.trim();
        if let Some(rest) = line.strip_prefix("m=") {
            lines += 1;
            video = rest.starts_with("video ");
            continue;
        }
        if !video {
            continue;
        }
        setup = match line.strip_prefix("a=setup:").map(str::trim) {
            Some("active") => "setup active",
            Some("passive") => "setup passive",
            Some("actpass") => "setup actpass",
            Some(_) => "setup other",
            None => setup,
        };
        direction = match line {
            "a=recvonly" => "recvonly",
            "a=sendonly" => "sendonly",
            "a=sendrecv" => "sendrecv",
            "a=inactive" => "inactive",
            _ => direction,
        };
    }
    format!("Answered the phone screen with H264 {profile}, {setup}, {direction}, {lines} media.")
}

fn answered_h264_profile(sdp: &str) -> Option<String> {
    let lines: Vec<&str> = sdp.split(['\r', '\n']).filter(|line| !line.is_empty()).collect();
    for line in &lines {
        let Some((pt, codec)) = rtpmap(line) else { continue };
        if !codec.eq_ignore_ascii_case("h264") {
            continue;
        }
        let params = lines.iter().find_map(|item| fmtp_params(item, &pt)).unwrap_or("");
        if fmtp_value(params, "apt").is_some() {
            continue;
        }
        let Some(profile) = fmtp_value(params, "profile-level-id") else { continue };
        if profile.len() == 6 && profile.bytes().all(|byte| byte.is_ascii_hexdigit()) {
            return Some(profile.to_ascii_lowercase());
        }
    }
    None
}

fn video_codec_names(sdp: &str) -> Vec<String> {
    let lines: Vec<&str> = sdp.split(['\r', '\n']).collect();
    let mut names = Vec::new();
    for line in &lines {
        let Some((pt, codec)) = rtpmap(line) else { continue };
        let name = match codec.to_ascii_uppercase().as_str() {
            "H264" => {
                let params = lines.iter().find_map(|item| fmtp_params(item, &pt)).unwrap_or("");
                match fmtp_value(params, "profile-level-id") {
                    Some(profile) if !profile.is_empty() => format!("H264 {profile}"),
                    _ => "H264".to_string(),
                }
            }
            "H265" | "HEVC" => "H265".to_string(),
            "VP8" => "VP8".to_string(),
            "VP9" => "VP9".to_string(),
            "AV1" => "AV1".to_string(),
            _ => continue,
        };
        if !names.contains(&name) {
            names.push(name);
        }
    }
    names
}

fn rtpmap(line: &str) -> Option<(String, String)> {
    let rest = line.strip_prefix("a=rtpmap:")?;
    let (pt, codec) = rest.split_once(' ')?;
    let codec = codec.split('/').next()?.trim();
    if pt.is_empty() || !pt.bytes().all(|byte| byte.is_ascii_digit()) {
        return None;
    }
    Some((pt.to_string(), codec.to_string()))
}

fn fmtp_apt(line: &str) -> Option<(String, String)> {
    let rest = line.strip_prefix("a=fmtp:")?;
    let (pt, params) = rest.split_once(' ')?;
    let apt = params.split(';').find_map(|part| part.trim().strip_prefix("apt="))?;
    let apt = apt.split_whitespace().next().unwrap_or(apt);
    if !pt.bytes().all(|byte| byte.is_ascii_digit()) || !apt.bytes().all(|byte| byte.is_ascii_digit()) {
        return None;
    }
    Some((pt.to_string(), apt.to_string()))
}

fn payload_attribute(line: &str) -> Option<String> {
    let rest = line.strip_prefix("a=")?;
    let (name, tail) = rest.split_once(':')?;
    if !matches!(name, "rtpmap" | "fmtp" | "rtcp-fb") {
        return None;
    }
    let pt = tail.split(|byte: char| !byte.is_ascii_digit()).next()?;
    if pt.is_empty() { None } else { Some(pt.to_string()) }
}

fn read_apps(value: Option<&Value>) -> Vec<AppView> {
    let Some(list) = value.and_then(Value::as_array) else { return Vec::new() };
    list.iter()
        .filter_map(|item| {
            let pkg = text_field(item, "pkg")?;
            Some(AppView {
                name: text_field(item, "name").unwrap_or_else(|| pkg.clone()),
                cmp: text_field(item, "cmp").unwrap_or_default(),
                user_id: item.get("userId").and_then(Value::as_i64).unwrap_or(0),
                icon_name: text_field(item, "iconName").unwrap_or_default(),
                pkg,
            })
        })
        .collect()
}

fn phone_share_from(body: &Value) -> Option<crate::nfs::PhoneShare> {
    let nfs_port = u16::try_from(json_i64(body.get("nfsport").or_else(|| body.get("nfsPort")))).ok()?;
    if nfs_port == 0 {
        return None;
    }
    let mount_port = u16::try_from(json_i64(body.get("port"))).unwrap_or(0);
    let export_path = text_field(body, "path").unwrap_or_else(|| "/sdcard".into());
    if !export_path.starts_with('/') || export_path.contains("..") || export_path.contains('\0') {
        return None;
    }
    let fingerprint = text_field(body, "fp")?;
    if fingerprint.len() != 64 || !fingerprint.chars().all(|ch| ch.is_ascii_hexdigit()) {
        return None;
    }
    Some(crate::nfs::PhoneShare { nfs_port, mount_port, export_path, fingerprint })
}

fn station_body(data: &Value) -> Option<Value> {
    match &data["data"] {
        Value::String(text) => serde_json::from_str(text).ok(),
        Value::Object(_) => Some(data["data"].clone()),
        _ => None,
    }
}

fn view_title(pkg: &str) -> String {
    match pkg {
        "mirror" => "Phone screen".into(),
        "desktop" => "Phone desktop".into(),
        "mobile" => "Phone".into(),
        other => other.to_string(),
    }
}

fn view_error(code: i64) -> &'static str {
    match code {
        145 | 147 => "The phone declined the screen or timed out.",
        293 => "The phone is already sharing with another computer.",
        296 => "The phone's video encoder is busy.",
        305 | 306 => "Screen sharing is turned off on the phone.",
        _ => "The phone could not start the screen.",
    }
}

fn stream_reason(code: i64) -> &'static str {
    match code {
        145 | 147 => "The phone declined the screen or timed out.",
        3857 => "The app closed on the phone.",
        3858 => "Another computer is using the phone screen.",
        3859 => "The phone has no encoder free.",
        3860 => "That app is not on the phone.",
        3861 => "The phone paused the screen.",
        _ => "The phone closed the screen.",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn clipboard_confirm_and_echo() {
        let mut desk = Desk::new();
        let effects = desk.handle(9, &json!({ "msgId": 1, "data": { "version": 1 } }));
        match &effects[0] {
            Effect::Send { service, data } => {
                assert_eq!(*service, 9);
                assert_eq!(data["msgId"], 2);
                assert_eq!(data["data"]["version"], 1);
            }
            _ => panic!("expected confirm"),
        }
        assert!(desk.clipboard_ready);
        let sent = desk.clipboard_out("hello").unwrap();
        assert_eq!(sent["data"]["text"], "hello");
        assert!(desk.clipboard_out("hello").is_none());
        let echo = desk.handle(9, &json!({ "msgId": 3, "data": { "clipId": 4, "text": "hello" } }));
        assert!(echo.is_empty());
        let empty = desk.handle(9, &json!({ "msgId": 3, "data": { "clipId": 0 } }));
        assert!(empty.is_empty());
    }

    #[test]
    fn notification_reply_and_removal() {
        let mut desk = Desk::new();
        let posted = json!({
            "version": "1.0",
            "key": "k1",
            "event": 0,
            "type": 0,
            "actions": [{ "action_id": 0, "action_name": "Reply", "remote_input": true }],
            "data": { "package_name": "com.example.chat", "apk_name": "Chat", "title": "Ada", "content": "Hi" }
        });
        let effects = desk.handle(5, &posted);
        assert!(effects.iter().any(|effect| matches!(effect, Effect::Banner { .. })));
        assert_eq!(desk.notifications().len(), 1);
        let again = desk.handle(5, &posted);
        assert!(again.is_empty());
        let reply = desk.reply("k1", "On my way").unwrap();
        match &reply[0] {
            Effect::Send { service, data } => {
                assert_eq!(*service, 5);
                assert_eq!(data["event"], 0);
                assert_eq!(data["data"]["reply_content"], "On my way");
                assert_eq!(data["data"]["smart_reply"], false);
            }
            _ => panic!("expected reply"),
        }
        desk.handle(5, &json!({ "key": "k1", "event": 2, "type": 0, "version": "1.0" }));
        assert!(desk.notifications().is_empty());
    }

    #[test]
    fn peer_start_uses_booleans() {
        let value = peer_start("com.example.notes", "com.example.notes.Main", 0, 0, 1700000000000);
        assert!(value["data"]["supportCustomDisplay"].is_boolean());
        assert_eq!(value["data"]["supportCustomDisplay"], false);
        let app = stream_start("com.example.notes", "com.example.notes.Main", 0, 0, 1700000000000);
        assert_eq!(app["data"]["supportCustomDisplay"], false);
        assert!(app["data"]["supportCustomDisplay"].is_boolean());
        assert_eq!(app["data"]["width"], 1920);
        assert_eq!(app["data"]["displayWidth"], 1920);
        let mirror = stream_start("mirror", "", 0, 0, 1700000000000);
        assert_eq!(mirror["data"]["supportCustomDisplay"], false);
        assert_eq!(value["data"]["vrSupportUnfolded"], false);
        assert_eq!(value["data"]["supportWebRTC"], 1);
        assert_eq!(value["data"]["pkg"], "com.example.notes");
    }

    #[test]
    fn cross_control_keeps_the_other_tiles() {
        let mut desk = Desk::new();
        let effects = desk.handle(26, &json!({ "ver": "1", "cmd": 2, "cid": 1, "code": 1, "width": 1080, "height": 2400 }));
        assert!(effects.iter().any(|effect| matches!(effect, Effect::Status)));
        assert!(desk.cross_active);
        assert_eq!(desk.cross_width, 1080);
    }

    #[test]
    fn offer_reaches_the_window_whole() {
        let sdp = "v=0\r\no=- 0 0 IN IP4 127.0.0.1\r\ns=-\r\nt=0 0\r\n\
m=video 9 UDP/TLS/RTP/SAVPF 96 97 98 99\r\n\
a=rtpmap:96 H265/90000\r\n\
a=fmtp:96 profile-id=1\r\n\
a=rtcp-fb:96 nack\r\n\
a=rtpmap:97 rtx/90000\r\n\
a=fmtp:97 apt=96\r\n\
a=rtpmap:98 H264/90000\r\n\
a=fmtp:98 packetization-mode=1;profile-level-id=42e01f\r\n\
a=rtcp-fb:98 nack pli\r\n\
a=rtpmap:99 rtx/90000\r\n\
a=fmtp:99 apt=98\r\n\
a=extmap:1 urn:3gpp:video-orientation\r\n\
a=sendonly\r\n";
        let wrapped = serde_json::to_string(&json!({ "type": "offer", "sdp": sdp })).unwrap();
        let mut desk = Desk::new();
        let effects = desk.handle(15, &json!({
            "ver": "1",
            "cmd": 3,
            "sessionId": "1700000000001",
            "pkg": "mirror",
            "userId": 0,
            "webRTCData": wrapped
        }));
        let offered = effects.iter().find_map(|effect| match effect {
            Effect::Offer { sdp, quiet, .. } => Some((sdp.clone(), *quiet)),
            _ => None,
        });
        let (sdp, quiet) = offered.expect("offer");
        assert!(!quiet);
        // The window answers the offer whole. Filtering it here makes the video section get rejected.
        assert!(sdp.contains("H264/90000"));
        assert!(sdp.contains("H265"));
        assert!(effects.iter().any(|effect| matches!(
            effect,
            Effect::Note(text) if text.contains("H265") && text.contains("H264 42e01f")
        )));
        let decoded = h264_only_sdp(&sdp);
        assert!(decoded.contains("H264/90000"));
        assert!(!decoded.contains("H265"));
        assert!(!decoded.contains("video-orientation"));
        assert!(decoded.contains("apt=98"));
        assert!(!decoded.contains("apt=96"));
        assert!(decoded.contains("m=video 9 UDP/TLS/RTP/SAVPF 98 99"));
        assert!(decoded.ends_with("\r\n"));
    }

    #[test]
    fn offer_keeps_every_h264_profile() {
        let sdp = "v=0\r\n\
m=video 9 UDP/TLS/RTP/SAVPF 96 97 98 99 100\r\n\
a=rtpmap:96 H264/90000\r\n\
a=fmtp:96 level-asymmetry-allowed=1;packetization-mode=1;profile-level-id=640c1f\r\n\
a=rtcp-fb:96 nack pli\r\n\
a=rtpmap:97 rtx/90000\r\n\
a=fmtp:97 apt=96\r\n\
a=rtpmap:98 H264/90000\r\n\
a=fmtp:98 level-asymmetry-allowed=1;packetization-mode=1;profile-level-id=42e01f\r\n\
a=rtcp-fb:98 nack pli\r\n\
a=rtpmap:99 rtx/90000\r\n\
a=fmtp:99 apt=98\r\n\
a=rtpmap:100 H265/90000\r\n";
        let out = h264_only_sdp(sdp);
        assert!(out.contains("640c1f"));
        assert!(out.contains("42e01f"));
        assert!(out.contains("apt=96"));
        assert!(out.contains("apt=98"));
        assert!(out.contains("rtpmap:96"));
        assert!(out.contains("rtpmap:98"));
        assert!(!out.contains("H265"));
        assert!(!out.contains("rtpmap:100"));
    }

    #[test]
    fn offer_keeps_high_when_it_is_the_only_h264() {
        let sdp = "v=0\r\n\
m=video 9 UDP/TLS/RTP/SAVPF 96 97\r\n\
a=rtpmap:96 H264/90000\r\n\
a=fmtp:96 packetization-mode=1;profile-level-id=640c1f\r\n\
a=rtpmap:97 rtx/90000\r\n\
a=fmtp:97 apt=96\r\n";
        let out = h264_only_sdp(sdp);
        assert!(out.contains("m=video 9 UDP/TLS/RTP/SAVPF 96 97"));
        assert!(out.contains("640c1f"));
    }

    #[test]
    fn answer_note_names_the_offered_h264_profile() {
        let sdp = "v=0\r\nm=video 9 UDP/TLS/RTP/SAVPF 96\r\n\
a=rtpmap:96 H264/90000\r\n\
a=fmtp:96 level-asymmetry-allowed=1;packetization-mode=1;profile-level-id=420033\r\n";
        assert_eq!(answered_h264_profile(sdp).as_deref(), Some("420033"));
        let mut desk = Desk::new();
        desk.sessions.insert("42".into(), Session {
            pkg: "mirror".into(),
            title: "Phone screen".into(),
            user_id: 0,
            display_id: -1,
            width: 0,
            height: 0,
            quiet: false,
            saw_offer: true,
            support_touch: false,
        });
        let effects = desk.answer("42", sdp).unwrap();
        let notes: Vec<_> = effects.iter().filter_map(|effect| match effect {
            Effect::Note(text) => Some(text.as_str()),
            _ => None,
        }).collect();
        assert_eq!(
            notes,
            vec!["Answered the phone screen with H264 420033, setup unset, direction unset, 1 media."]
        );
        let answer = effects.iter().find_map(|effect| match effect {
            Effect::Send { service, data } if *service == 15 && data["cmd"] == 4 => Some(data),
            _ => None,
        }).expect("answer");
        assert!(answer.get("sessionId").is_none());
        assert!(answer.get("webRTCData").is_none());
        assert_eq!(answer["data"]["sessionId"], "42");
        assert_eq!(answer["data"]["error"], 0);
        let web = answer["data"]["webRTCData"].as_str().expect("webRTCData string");
        assert!(web.contains("420033"));
        let ice = desk.ice("42", "candidate:1 1 udp 1 192.168.1.8 50000 typ host", 0, "0").expect("ice");
        match ice {
            Effect::Send { service, data } => {
                assert_eq!(service, 15);
                assert_eq!(data["cmd"], 20);
                assert!(data.get("sessionId").is_none());
                assert_eq!(data["data"]["sessionId"], "42");
                assert_eq!(data["data"]["type"], "candidate");
                let body: serde_json::Value = serde_json::from_str(data["data"]["data"].as_str().unwrap()).unwrap();
                assert_eq!(body["id"], "0");
                assert!(body["candidate"].as_str().unwrap().contains("192.168.1.8"));
            }
            _ => panic!("expected ice send"),
        }
    }

    #[test]
    fn offer_names_each_h264_profile() {
        let sdp = "v=0\r\nm=video 9 UDP/TLS/RTP/SAVPF 96 98\r\n\
a=rtpmap:96 H264/90000\r\na=fmtp:96 packetization-mode=1;profile-level-id=640c1f\r\n\
a=rtpmap:98 H264/90000\r\na=fmtp:98 packetization-mode=1;profile-level-id=42e01f\r\n";
        let names = video_codec_names(sdp);
        assert_eq!(names, vec!["H264 640c1f".to_string(), "H264 42e01f".to_string()]);
    }

    #[test]
    fn offer_accepts_webrtc_object_and_leaves_h265_only() {
        let sdp = "v=0\r\nm=video 9 UDP/TLS/RTP/SAVPF 96\r\na=rtpmap:96 H265/90000\r\n";
        let mut desk = Desk::new();
        let effects = desk.handle(15, &json!({
            "ver": "1",
            "cmd": 3,
            "data": {
                "sessionId": 42,
                "pkg": "mirror",
                "webRTCData": { "type": "offer", "sdp": sdp }
            }
        }));
        let offered = effects.iter().find_map(|effect| match effect {
            Effect::Offer { sdp, .. } => Some(sdp.clone()),
            _ => None,
        }).expect("offer");
        assert!(offered.contains("H265"));
        let ice = desk.handle(15, &json!({
            "ver": "1",
            "cmd": 20,
            "sessionId": 42,
            "data": { "type": "candidate", "label": 0, "id": "0", "candidate": "candidate:1 1 udp 1 192.168.1.35 50001 typ host" }
        }));
        match &ice[0] {
            Effect::Ice { candidate, label, mid, .. } => {
                assert!(candidate.contains("192.168.1.35"));
                assert_eq!(*label, 0);
                assert_eq!(mid, "0");
            }
            _ => panic!("expected ice"),
        }
        let raw = desk.handle(15, &json!({
            "ver": "1",
            "cmd": 20,
            "sessionId": 42,
            "data": "a=candidate:2 1 udp 1 192.168.1.35 50002 typ host",
            "label": "1",
            "id": 1
        }));
        match &raw[0] {
            Effect::Ice { candidate, label, mid, .. } => {
                assert!(candidate.starts_with("candidate:"));
                assert!(!candidate.starts_with("a="));
                assert_eq!(*label, 1);
                assert_eq!(mid, "1");
            }
            _ => panic!("expected raw ice"),
        }
    }

    #[test]
    fn a_second_offer_does_not_restart_the_session() {
        let sdp = "v=0\r\nm=video 9 UDP/TLS/RTP/SAVPF 96\r\na=rtpmap:96 H264/90000\r\n";
        let wrapped = serde_json::to_string(&json!({ "type": "offer", "sdp": sdp })).unwrap();
        let mut desk = Desk::new();
        let first = desk.handle(15, &json!({
            "ver": "1",
            "cmd": 3,
            "data": { "sessionId": 7, "pkg": "com.example.notes", "webRTCData": wrapped }
        }));
        assert!(first.iter().any(|effect| matches!(effect, Effect::Offer { .. })));
        let second = desk.handle(15, &json!({
            "ver": "1",
            "cmd": 3,
            "data": {
                "sessionId": 7,
                "pkg": "com.example.notes",
                "displayId": 4,
                "width": 1600,
                "height": 900,
                "webRTCData": wrapped
            }
        }));
        assert!(second.iter().all(|effect| !matches!(effect, Effect::Offer { .. })));
        assert_eq!(desk.sessions.get("7").map(|session| session.display_id), Some(4));
        assert_eq!(desk.sessions.get("7").map(|session| session.width), Some(1600));
    }

    #[test]
    fn an_app_screen_records_that_it_takes_touch() {
        let sdp = "v=0\r\nm=video 9 UDP/TLS/RTP/SAVPF 96\r\na=rtpmap:96 H264/90000\r\n";
        let wrapped = serde_json::to_string(&json!({ "type": "offer", "sdp": sdp })).unwrap();
        let mut desk = Desk::new();
        desk.handle(15, &json!({
            "ver": "1",
            "cmd": 3,
            "data": { "sessionId": 7, "pkg": "com.example.notes", "webRTCData": wrapped }
        }));
        let effects = desk.handle(15, &json!({
            "ver": "1",
            "cmd": 8,
            "data": {
                "state": 1,
                "sessionId": 7,
                "displayId": 4,
                "pkg": "com.example.notes",
                "supportTouch": true
            }
        }));
        assert!(effects.iter().any(|effect| matches!(effect, Effect::Note(text) if text.contains("finger"))));
        let view = desk.sessions().into_iter().find(|session| session.session_id == "7").expect("session");
        assert!(view.support_touch);
        assert_eq!(view.display_id, 4);
        let again = desk.handle(15, &json!({
            "ver": "1",
            "cmd": 8,
            "data": { "state": 1, "sessionId": 7, "displayId": 4, "supportTouch": true }
        }));
        assert!(again.iter().all(|effect| !matches!(effect, Effect::Note(text) if text.contains("finger"))));
    }

    #[test]
    fn a_missed_start_can_be_retried() {
        let mut desk = Desk::new();
        let effects = desk.start_mirror().unwrap();
        let token = effects.iter().find_map(|effect| match effect {
            Effect::WatchStart { token } => Some(*token),
            _ => None,
        }).unwrap();
        assert!(desk.give_up(token));
        assert!(desk.start_mirror().is_ok());
        assert!(!desk.give_up(token));
    }

    #[test]
    fn start_mirror_asks_for_the_view_and_opens_it() {
        let mut desk = Desk::new();
        let effects = desk.start_mirror().expect("start");
        let sends: Vec<_> = effects
            .iter()
            .filter_map(|effect| match effect {
                Effect::Send { service, data } => Some((*service, data.clone())),
                _ => None,
            })
            .collect();
        assert_eq!(sends.len(), 2);
        assert_eq!(sends[0].0, 1);
        assert_eq!(sends[0].1["cmd"], 1);
        assert_eq!(sends[0].1["reply"], 0);
        assert_eq!(sends[1].0, 15);
        assert_eq!(sends[1].1["cmd"], 1);
        assert_eq!(sends[1].1["data"]["pkg"], "mirror");
        assert_eq!(sends[1].1["data"]["source"], 0);
        assert!(sends[1].1["data"].get("cmp").is_none());
        assert!(sends[1].1["data"].get("shortcutId").is_none());
        assert_eq!(sends[1].1["data"]["supportCustomDisplay"], false);
        assert!(sends[1].1["data"]["supportCustomDisplay"].is_boolean());
        let again = desk.start_mirror().expect("second tap waits");
        assert!(again.iter().all(|effect| !matches!(effect, Effect::Send { .. })));
        let echo = desk.handle(15, &json!({
            "ver": "1",
            "cmd": 2,
            "data": { "pkg": "mirror", "source": 1 }
        }));
        assert!(echo.iter().all(|effect| !matches!(effect, Effect::Send { .. })));
    }

    #[test]
    fn webcam_start_has_no_reply_flag_and_keeps_the_offer() {
        let mut desk = Desk::new();
        let start = desk.start_webcam().expect("start");
        let request = start.iter().find_map(|effect| match effect {
            Effect::Send { service: 3, data } => Some(data.clone()),
            _ => None,
        }).expect("request");
        assert_eq!(request["cmd"], 0);
        assert!(request["data"].get("isReply").is_none());
        assert_eq!(request["data"]["heartbeat"], 1);
        assert_eq!(request["data"]["minL"], 1);
        assert_eq!(request["data"]["lite"], 0);
        assert!(desk.webcam_heartbeat());
        let accepted = desk.handle(3, &json!({
            "ver": "1",
            "cmd": 0,
            "data": "{\"isReply\":1,\"heartbeat\":1}"
        }));
        assert!(accepted.iter().any(|effect| matches!(effect, Effect::Send { service: 3, data } if data["cmd"] == 1 && data["data"]["port"] == 6466 && data["data"]["heartbeat"] == 1)));
        let sdp = "v=0\r\nm=video 9 UDP/TLS/RTP/SAVPF 96 98\r\na=rtpmap:96 H264/90000\r\na=rtpmap:98 VP8/90000\r\n";
        let wrapped = serde_json::to_string(&json!({ "type": "offer", "sdp": sdp })).unwrap();
        let effects = desk.handle(3, &json!({
            "ver": "1",
            "cmd": 1,
            "data": { "isWebRTC": 1, "sessionId": 42, "webRTCData": wrapped }
        }));
        let offer = effects.iter().find_map(|effect| match effect {
            Effect::Offer { sdp, pkg, session_id, .. } => Some((sdp.clone(), pkg.clone(), session_id.clone())),
            _ => None,
        }).expect("offer");
        assert_eq!(offer.1, "webcam");
        assert_eq!(offer.2, "42");
        assert!(offer.0.contains("VP8"));
        assert!(desk.webcam_flip().is_ok());
        let answer = desk.answer("42", "v=0\r\nm=video 9 UDP/TLS/RTP/SAVPF 96\r\n").expect("answer");
        let sent = answer.iter().find_map(|effect| match effect {
            Effect::Send { service: 3, data } if data["cmd"] == 17 => Some(data.clone()),
            _ => None,
        }).expect("cmd 17");
        assert!(sent["data"]["sessionId"].is_number());
        assert_eq!(sent["data"]["sessionId"], 42);
        let ice = desk.ice("42", "candidate:1 1 udp 1 192.168.1.4 9 typ host", 0, "0").expect("ice");
        match ice {
            Effect::Send { data, .. } => assert_eq!(data["data"]["sessionId"], 42),
            _ => panic!("ice"),
        }
        let mut early = Desk::new();
        early.start_webcam().unwrap();
        let stop = early.stop_pkg("webcam");
        assert!(start.iter().any(|effect| matches!(effect, Effect::Obs { on: true })));
        assert!(stop.iter().any(|effect| matches!(effect, Effect::Send { service: 3, data } if data["cmd"] == 2)));
        assert!(stop.iter().any(|effect| matches!(effect, Effect::Obs { on: false })));
        assert!(!early.webcam_heartbeat());
    }

    #[test]
    fn start_desktop_asks_for_the_desktop_view() {
        let mut desk = Desk::new();
        let effects = desk.start_desktop().expect("start");
        let sends: Vec<_> = effects
            .iter()
            .filter_map(|effect| match effect {
                Effect::Send { service, data } => Some((*service, data.clone())),
                _ => None,
            })
            .collect();
        assert_eq!(sends[0].0, 1);
        assert_eq!(sends[0].1["view"], 2);
        assert_eq!(sends[0].1["reply"], 0);
        assert_eq!(sends[1].1["data"]["pkg"], "desktop");
        assert_eq!(sends[1].1["data"]["supportCustomDisplay"], false);
    }

    #[test]
    fn folder_reply_names_the_phone_share() {
        let mut desk = Desk::new();
        let request = Desk::phone_browse_request("ab");
        assert_eq!(request["cmd"], 1001);
        let inner: serde_json::Value = serde_json::from_str(request["data"].as_str().unwrap()).unwrap();
        assert_eq!(inner["type"], 0);
        assert_eq!(inner["dt2type"], 2);
        assert_eq!(inner["fp"], "AB");
        let effects = desk.handle(168, &json!({
            "ver": "1.0",
            "cmd": 1001,
            "data": "{\"type\":1,\"path\":\"/sdcard\",\"port\":111,\"nfsport\":2049,\"fp\":\"0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef\",\"nfs_version\":8}"
        }));
        assert!(effects.iter().any(|effect| matches!(effect, Effect::ShareReady)));
        let share = desk.phone_share().expect("share");
        assert_eq!(share.nfs_port, 2049);
        assert_eq!(share.export_path, "/sdcard");
        let asking = desk.handle(168, &json!({
            "cmd": 1001,
            "data": { "type": 0, "fp": "phone" }
        }));
        assert!(asking.iter().any(|effect| matches!(effect, Effect::Note(_))));
        assert!(desk.handle(168, &json!({ "cmd": 1001, "data": { "type": 0 } })).is_empty());
    }
}
