//! Phone texts over service 204. The phone is the messenger. This Mac only asks
//! for the recent list and hands a new text back to the phone to send.
//!
//! Nothing in here is written to the activity log except fixed sentences.
//! Message text, numbers, and contact names stay in memory for the panel.

use serde::Serialize;
use serde_json::{json, Value};
use std::collections::{HashMap, VecDeque};

const KEEP: usize = 800;
const MARK_LIMIT: usize = 40;

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SmsView {
    /// waiting, denied, off, loading, or ready.
    pub phase: String,
    pub unread: i64,
    pub threads: Vec<ThreadView>,
    pub open_key: String,
    pub open_title: String,
    pub open_address: String,
    pub open: Vec<MessageView>,
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ThreadView {
    pub key: String,
    pub title: String,
    pub address: String,
    pub preview: String,
    pub date: i64,
    pub unread: i64,
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MessageView {
    pub id: String,
    pub outgoing: bool,
    pub body: String,
    pub date: i64,
    pub pending: bool,
    pub failed: bool,
}

#[derive(Default)]
pub struct Outcome {
    pub sends: Vec<Value>,
    pub notes: Vec<String>,
    pub changed: bool,
}

struct Item {
    id: String,
    thread: String,
    address: String,
    outgoing: bool,
    body: String,
    date: i64,
    read: bool,
    pending: bool,
    failed: bool,
    kind: i64,
    request_id: i64,
}

pub struct Sms {
    phone: Option<String>,
    granted: bool,
    enabled: bool,
    seen_status: bool,
    ready_sent: bool,
    asked: bool,
    mms_asked: bool,
    limited_asked: bool,
    history_done: bool,
    announced: bool,
    noted_denied: bool,
    noted_off: bool,
    unread_hint: i64,
    names: HashMap<String, String>,
    own: Vec<String>,
    items: HashMap<String, Item>,
    order: VecDeque<String>,
    open: Option<String>,
    next_request: i64,
}

impl Sms {
    pub fn new() -> Self {
        Self {
            phone: None,
            granted: false,
            enabled: false,
            seen_status: false,
            ready_sent: false,
            asked: false,
            mms_asked: false,
            limited_asked: false,
            history_done: false,
            announced: false,
            noted_denied: false,
            noted_off: false,
            unread_hint: 0,
            names: HashMap::new(),
            own: Vec::new(),
            items: HashMap::new(),
            order: VecDeque::new(),
            open: None,
            next_request: 1,
        }
    }

    /// A new phone drops the previous phone's texts. The same phone keeps them
    /// and is asked again, because the phone only answers after a fresh ready.
    pub fn on_phone(&mut self, real_id: &str) {
        if self.phone.as_deref() != Some(real_id) {
            let announced = false;
            *self = Self::new();
            self.phone = Some(real_id.to_string());
            self.announced = announced;
            return;
        }
        self.ready_sent = false;
        self.asked = false;
        self.mms_asked = false;
        self.limited_asked = false;
        self.history_done = false;
        self.noted_denied = false;
        self.noted_off = false;
    }

    pub fn clear(&mut self) {
        *self = Self::new();
    }

    pub fn view(&self) -> SmsView {
        let mut groups: HashMap<String, (i64, i64, String, String)> = HashMap::new();
        for id in &self.order {
            let Some(item) = self.items.get(id) else { continue };
            let entry = groups.entry(item.thread.clone()).or_insert((0, 0, String::new(), String::new()));
            if !item.read && !item.outgoing {
                entry.0 += 1;
            }
            if item.date >= entry.1 {
                entry.1 = item.date;
                entry.2 = item.address.clone();
                entry.3 = preview_of(item);
            }
        }
        let mut threads: Vec<ThreadView> = groups
            .into_iter()
            .map(|(key, (unread, date, address, preview))| ThreadView {
                title: self.names.get(&key).cloned().filter(|name| !name.is_empty()).unwrap_or_else(|| address.clone()),
                key,
                address,
                preview,
                date,
                unread,
            })
            .collect();
        threads.sort_by(|a, b| b.date.cmp(&a.date).then_with(|| a.title.cmp(&b.title)));
        threads.truncate(80);
        let counted = threads.iter().map(|thread| thread.unread).sum::<i64>();
        // The phone's unread count is only a hint before any text has arrived.
        // Once the list is in, the badge follows the conversations on screen.
        let unread = if self.items.is_empty() && !self.history_done { self.unread_hint.max(counted) } else { counted };
        let open_key = self.open.clone().unwrap_or_default();
        let open_thread = threads.iter().find(|thread| thread.key == open_key);
        let open_title = open_thread.map(|thread| thread.title.clone()).unwrap_or_default();
        let open_address = open_thread.map(|thread| thread.address.clone()).unwrap_or_else(|| {
            self.items.values().find(|item| item.thread == open_key).map(|item| item.address.clone()).unwrap_or_default()
        });
        let mut open: Vec<MessageView> = self
            .items
            .values()
            .filter(|item| item.thread == open_key && !open_key.is_empty())
            .map(|item| MessageView {
                id: item.id.clone(),
                outgoing: item.outgoing,
                body: item.body.clone(),
                date: item.date,
                pending: item.pending,
                failed: item.failed,
            })
            .collect();
        open.sort_by(|a, b| a.date.cmp(&b.date).then_with(|| a.id.cmp(&b.id)));
        if open.len() > 200 {
            open = open.split_off(open.len() - 200);
        }
        SmsView {
            phase: self.phase().to_string(),
            unread,
            threads,
            open_key,
            open_title,
            open_address,
            open,
        }
    }

    pub fn on_message(&mut self, service: i64, cmd: i64, data: &Value) -> Outcome {
        if service == 100 && cmd == 1018 {
            return self.on_status(data);
        }
        if service != 204 {
            return Outcome::default();
        }
        // The phone wraps every reply: {ver, cmd, pId, data: <list or bean>}.
        // The list itself is one level down. A bare array is the older shape.
        let inner = unwrap_payload(data);
        match cmd {
            2 | 3 | 25 => self.on_contacts(&inner),
            5 => self.on_accept(&inner),
            6 => self.on_result(&inner),
            7 => self.on_changed(&inner),
            11 | 23 | 29 => self.on_batch(&inner),
            13 => self.on_own(&inner),
            14 => self.on_deleted(&inner),
            17 | 21 => self.on_status(&inner),
            _ => Outcome::default(),
        }
    }

    pub fn close(&mut self) -> Outcome {
        if self.open.take().is_none() {
            return Outcome::default();
        }
        Outcome { changed: true, ..Outcome::default() }
    }

    pub fn open(&mut self, key: &str) -> Result<Outcome, String> {
        let key = key.trim();
        if key.is_empty() || !self.items.values().any(|item| item.thread == key) {
            return Err("That conversation is not on this Mac.".into());
        }
        self.open = Some(key.to_string());
        let mut outcome = self.mark_thread(key);
        outcome.changed = true;
        Ok(outcome)
    }

    pub fn send_open(&mut self, text: &str) -> Result<Outcome, String> {
        let key = self.open.clone().ok_or_else(|| "Open a conversation first.".to_string())?;
        let address = self
            .items
            .values()
            .filter(|item| item.thread == key)
            .max_by_key(|item| item.date)
            .map(|item| item.address.clone())
            .filter(|address| !address.is_empty())
            .ok_or_else(|| "That conversation has no number.".to_string())?;
        self.queue_send(&address, text)
    }

    pub fn send_new(&mut self, address: &str, text: &str) -> Result<Outcome, String> {
        let address = clean_address(address)?;
        let outcome = self.queue_send(&address, text)?;
        self.open = Some(thread_key(&address));
        Ok(outcome)
    }

    fn phase(&self) -> &'static str {
        if !self.seen_status {
            "waiting"
        } else if !self.granted {
            "denied"
        } else if !self.enabled {
            "off"
        } else if !self.history_done {
            "loading"
        } else {
            "ready"
        }
    }

    fn on_status(&mut self, data: &Value) -> Outcome {
        self.seen_status = true;
        if let Some(unread) = data.get("unreadCnt").and_then(Value::as_i64) {
            self.unread_hint = unread.max(0);
        }
        if let Some(granted) = data.get("granted").and_then(Value::as_bool) {
            self.granted = granted;
        }
        let enabled = data.get("enabled").and_then(Value::as_bool).unwrap_or(self.enabled);
        let became = enabled && !self.enabled;
        self.enabled = enabled;
        let mut outcome = Outcome { changed: true, ..Outcome::default() };
        if !self.granted {
            if !self.noted_denied {
                self.noted_denied = true;
                outcome.notes.push("The phone has not allowed messages.".into());
            }
            return outcome;
        }
        if !self.enabled {
            if !self.ready_sent {
                self.ready_sent = true;
                outcome.sends.push(frame(18, None));
                outcome.notes.push("Asked the phone to turn messages on for this Mac.".into());
            } else if !self.noted_off {
                self.noted_off = true;
                outcome.notes.push("Turn on Messages for this Mac in the phone's Smart Connect settings.".into());
            }
            return outcome;
        }
        if !self.asked || became {
            self.asked = true;
            outcome.sends.push(frame(12, Some(json!({}))));
            outcome.sends.push(frame(1, Some(json!({}))));
            outcome.sends.push(frame(28, Some(json!({ "type": 1, "count": 0 }))));
            outcome.notes.push("Asked the phone for messages.".into());
        }
        outcome
    }

    fn on_contacts(&mut self, data: &Value) -> Outcome {
        let mut changed = false;
        for contact in list_or_beans(data, &["contactList", "contacts", "contactBeanList", "list"]) {
            let name = text_field(contact, &["name", "displayName"]).unwrap_or_default();
            let name = name.trim();
            if name.is_empty() {
                continue;
            }
            for number in contact_numbers(contact) {
                let key = thread_key(&number);
                if key.is_empty() {
                    continue;
                }
                self.names.insert(key, name.to_string());
                changed = true;
            }
        }
        Outcome { changed, ..Outcome::default() }
    }

    fn on_own(&mut self, data: &Value) -> Outcome {
        for number in beans(data, &["phoneNumberList", "list", "numbers", "simList"])
            .into_iter()
            .flat_map(|item| {
                text_field(item, &["phoneNumber", "number", "address"])
                    .into_iter()
                    .chain(contact_numbers(item))
            })
            .chain(text_field(data, &["phoneNumber"]).into_iter())
        {
            let key = thread_key(&number);
            if !key.is_empty() && !self.own.contains(&key) {
                self.own.push(key);
            }
        }
        Outcome::default()
    }

    fn on_batch(&mut self, data: &Value) -> Outcome {
        let kind = data.get("type").and_then(Value::as_i64).unwrap_or(1);
        let added = self.ingest_list(data, kind);
        let mut outcome = Outcome { changed: true, ..Outcome::default() };
        let finished = match data {
            Value::Array(_) => true,
            Value::Object(_) => last_batch(data) && (data.get("messageBeanList").is_some() || data.get("totalBatches").is_some() || data.get("currentBatch").is_some() || added),
            _ => false,
        };
        if kind == 1 && finished {
            if self.items.is_empty() && self.unread_hint > 0 && !self.limited_asked {
                self.limited_asked = true;
                outcome.sends.push(frame(10, Some(json!({ "type": 1, "count": 200 }))));
                outcome.notes.push("Asked the phone for messages.".into());
                return outcome;
            }
            self.history_done = true;
            if !self.announced {
                self.announced = true;
                outcome.notes.push("Messages are on.".into());
            }
            if !self.mms_asked {
                self.mms_asked = true;
                outcome.sends.push(frame(28, Some(json!({ "type": 2, "count": 0 }))));
            }
        }
        let _ = added;
        outcome
    }

    fn on_changed(&mut self, data: &Value) -> Outcome {
        let action = data.get("action").and_then(Value::as_i64).unwrap_or(1);
        let kind = data.get("type").and_then(Value::as_i64).unwrap_or(1);
        if action == 2 {
            return self.on_deleted(data);
        }
        let before = self.items.values().filter(|item| !item.outgoing).count();
        self.ingest_list(data, kind);
        let mut outcome = Outcome { changed: true, ..Outcome::default() };
        if self.items.values().filter(|item| !item.outgoing).count() > before {
            outcome.notes.push("A new message arrived.".into());
        }
        if let Some(key) = self.open.clone() {
            let marks = self.mark_thread(&key);
            outcome.sends.extend(marks.sends);
        }
        outcome
    }

    fn on_deleted(&mut self, data: &Value) -> Outcome {
        let mut changed = false;
        for id in delete_ids(data) {
            if self.items.remove(&id).is_some() {
                self.order.retain(|existing| existing != &id);
                changed = true;
            }
        }
        Outcome { changed, ..Outcome::default() }
    }

    fn on_accept(&mut self, data: &Value) -> Outcome {
        let Some(request) = data.get("requestId").and_then(Value::as_i64) else {
            return Outcome::default();
        };
        let mut changed = false;
        for item in self.items.values_mut() {
            if item.request_id == request && item.pending {
                item.pending = false;
                changed = true;
            }
        }
        Outcome { changed, ..Outcome::default() }
    }

    fn on_result(&mut self, data: &Value) -> Outcome {
        let request = data.get("requestId").and_then(Value::as_i64);
        let failed = send_failed(data);
        let new_id = data.get("msgId").map(value_id).unwrap_or_default();
        let mut changed = false;
        let local = request.and_then(|request| {
            self.items.iter().find(|(_, item)| item.request_id == request && item.id.starts_with('p')).map(|(id, _)| id.clone())
        });
        if let Some(local) = local {
            if let Some(mut item) = self.items.remove(&local) {
                self.order.retain(|id| id != &local);
                item.pending = false;
                item.failed = failed;
                if !new_id.is_empty() {
                    item.id = new_id.clone();
                }
                let id = item.id.clone();
                self.items.insert(id.clone(), item);
                self.order.push_back(id);
                changed = true;
            }
        }
        let mut outcome = Outcome { changed, ..Outcome::default() };
        if failed {
            outcome.notes.push("The phone could not send that text.".into());
        } else if changed {
            outcome.notes.push("Sent.".into());
        }
        outcome
    }

    fn queue_send(&mut self, address: &str, text: &str) -> Result<Outcome, String> {
        if !self.granted {
            return Err(if self.seen_status {
                "The phone has not allowed messages.".into()
            } else {
                "Connect the phone first.".into()
            });
        }
        if !self.enabled {
            return Err("Turn on Messages for this Mac on the phone.".into());
        }
        let body = text.trim();
        if body.is_empty() {
            return Err("Enter a message.".into());
        }
        if body.chars().count() > 1000 {
            return Err("That message is too long.".into());
        }
        let request = self.next_request;
        self.next_request = self.next_request.saturating_add(1);
        let date = now_ms();
        let id = format!("p{request}");
        let item = Item {
            id: id.clone(),
            thread: thread_key(address),
            address: address.to_string(),
            outgoing: true,
            body: body.to_string(),
            date,
            read: true,
            pending: true,
            failed: false,
            kind: 1,
            request_id: request,
        };
        self.order.push_back(id.clone());
        self.items.insert(id, item);
        self.trim();
        let payload = frame(
            4,
            Some(json!({
                "requestId": request,
                "msgId": request,
                "subId": -1,
                "messageType": 2,
                "toAddress": [address],
                "body": body,
                "medias": [],
                "read": 1,
                "date": date
            })),
        );
        Ok(Outcome {
            sends: vec![payload],
            changed: true,
            ..Outcome::default()
        })
    }

    fn mark_thread(&mut self, key: &str) -> Outcome {
        let mut sends = Vec::new();
        for item in self.items.values_mut() {
            if item.thread != key || item.outgoing || item.read || item.id.starts_with('p') {
                continue;
            }
            item.read = true;
            if sends.len() >= MARK_LIMIT {
                continue;
            }
            if let Ok(msg_id) = item.id.parse::<i64>() {
                sends.push(frame(8, Some(json!({ "msgId": msg_id, "status": 1, "type": item.kind }))));
            } else {
                sends.push(frame(8, Some(json!({ "msgId": item.id, "status": 1, "type": item.kind }))));
            }
        }
        Outcome { sends, changed: true, ..Outcome::default() }
    }

    fn ingest_list(&mut self, data: &Value, kind: i64) -> bool {
        let mut added = false;
        for bean in list_or_beans(data, &["messageBeanList", "messages", "messageList", "smsList", "list"]) {
            if self.ingest_one(bean, kind) {
                added = true;
            }
        }
        self.trim();
        added
    }

    fn ingest_one(&mut self, bean: &Value, kind: i64) -> bool {
        let id = bean.get("msgId").map(value_id).unwrap_or_default();
        if id.is_empty() {
            return false;
        }
        let request_id = bean.get("requestId").and_then(Value::as_i64).unwrap_or(0);
        if request_id > 0 {
            if let Some(local) = self
                .items
                .iter()
                .find(|(_, item)| item.request_id == request_id && item.id.starts_with('p'))
                .map(|(id, _)| id.clone())
            {
                self.items.remove(&local);
                self.order.retain(|existing| existing != &local);
            }
        }
        let outgoing = bean.get("messageType").and_then(Value::as_i64).unwrap_or(1) == 2;
        let from = first_address(bean.get("fromAddress"));
        let to = first_address(bean.get("toAddress"));
        let (address, other) = if outgoing { (to, from) } else { (from, to) };
        let address = if thread_key(&address).is_empty() || self.own.iter().any(|own| own == &thread_key(&address)) {
            if thread_key(&other).is_empty() { address } else { other }
        } else {
            address
        };
        if thread_key(&address).is_empty() {
            return false;
        }
        let body = text_field(bean, &["body", "text", "content"]).unwrap_or_default();
        let subject = text_field(bean, &["subject"]).unwrap_or_default();
        let photo = has_media(bean);
        let body = if body.trim().is_empty() && photo {
            "Photo".into()
        } else if body.trim().is_empty() && !subject.trim().is_empty() {
            subject.trim().chars().take(1000).collect()
        } else {
            body.trim().chars().take(1000).collect()
        };
        let date = bean.get("date").and_then(Value::as_i64).unwrap_or_else(now_ms);
        let read = match bean.get("read") {
            Some(Value::Bool(value)) => *value,
            Some(Value::Number(value)) => value.as_i64().unwrap_or(1) != 0,
            _ => outgoing,
        };
        let thread = thread_key(&address);
        let fresh = !self.items.contains_key(&id);
        if fresh {
            self.order.push_back(id.clone());
        }
        let pending = self.items.get(&id).is_some_and(|item| item.pending);
        let failed = self.items.get(&id).is_some_and(|item| item.failed);
        self.items.insert(
            id.clone(),
            Item {
                id,
                thread,
                address,
                outgoing,
                body,
                date,
                read,
                pending,
                failed,
                kind: if kind == 2 { 2 } else { 1 },
                request_id,
            },
        );
        fresh
    }

    fn trim(&mut self) {
        while self.order.len() > KEEP {
            if let Some(id) = self.order.pop_front() {
                self.items.remove(&id);
            }
        }
    }
}

/// Last 10 digits for a phone number, so a country code and the local form match.
/// Anything else is grouped by its letters and digits.
pub fn thread_key(address: &str) -> String {
    let trimmed = address.trim();
    if trimmed.is_empty() {
        return String::new();
    }
    let phoneish = trimmed.chars().all(|ch| ch.is_ascii_digit() || ch.is_whitespace() || matches!(ch, '+' | '-' | '(' | ')'));
    let digits: String = trimmed.chars().filter(|ch| ch.is_ascii_digit()).collect();
    if phoneish && digits.len() >= 6 {
        let start = digits.len().saturating_sub(10);
        return digits[start..].to_string();
    }
    trimmed.chars().filter(|ch| ch.is_ascii_alphanumeric()).flat_map(|ch| ch.to_lowercase()).collect()
}

fn clean_address(raw: &str) -> Result<String, String> {
    let trimmed = raw.trim();
    let digits = trimmed.chars().filter(|ch| ch.is_ascii_digit()).count();
    let ok = trimmed.chars().all(|ch| ch.is_ascii_digit() || matches!(ch, '+' | '-' | '(' | ')' | ' '));
    if !ok || digits < 3 || digits > 15 || trimmed.chars().count() > 32 {
        return Err("Enter a phone number.".into());
    }
    let mut out = String::new();
    if trimmed.starts_with('+') {
        out.push('+');
    }
    out.extend(trimmed.chars().filter(|ch| ch.is_ascii_digit()));
    Ok(out)
}

fn frame(cmd: i64, data: Option<Value>) -> Value {
    let mut message = json!({ "ver": "5.0", "cmd": cmd, "pId": 110 });
    if let Some(data) = data {
        message["data"] = data;
    }
    message
}

/// A reply is `{ver, cmd, pId, data: ...}`. The useful part is `data`, which is
/// an object for a sync batch and a bare array on older phones.
fn unwrap_payload(data: &Value) -> Value {
    match data.get("data") {
        Some(Value::String(text)) => serde_json::from_str(text).unwrap_or_else(|_| data.clone()),
        Some(Value::Object(_)) | Some(Value::Array(_)) => data["data"].clone(),
        _ => data.clone(),
    }
}

fn list_or_beans<'a>(data: &'a Value, keys: &[&str]) -> Vec<&'a Value> {
    if let Value::Array(items) = data {
        return items.iter().collect();
    }
    beans(data, keys)
}

fn beans<'a>(data: &'a Value, keys: &[&str]) -> Vec<&'a Value> {
    for key in keys {
        if let Some(Value::Array(items)) = data.get(*key) {
            return items.iter().collect();
        }
    }
    if data.get("msgId").is_some() || data.get("name").is_some() || data.get("phoneNumber").is_some() {
        return vec![data];
    }
    Vec::new()
}

fn text_field(value: &Value, keys: &[&str]) -> Option<String> {
    for key in keys {
        match value.get(*key) {
            Some(Value::String(text)) if !text.trim().is_empty() => return Some(text.clone()),
            Some(Value::Number(number)) => return Some(number.to_string()),
            _ => {}
        }
    }
    None
}

fn contact_numbers(contact: &Value) -> Vec<String> {
    let mut numbers = Vec::new();
    for key in ["phone", "phones", "phoneNumber", "number", "tel"] {
        match contact.get(key) {
            Some(Value::String(text)) => numbers.extend(split_addresses(text)),
            Some(Value::Array(items)) => {
                for item in items {
                    match item {
                        Value::String(text) => numbers.extend(split_addresses(text)),
                        Value::Object(_) => {
                            if let Some(text) = text_field(item, &["number", "phoneNumber", "address", "phone"]) {
                                numbers.extend(split_addresses(&text));
                            }
                        }
                        _ => {}
                    }
                }
            }
            _ => {}
        }
    }
    numbers
}

fn first_address(value: Option<&Value>) -> String {
    let Some(value) = value else { return String::new() };
    match value {
        Value::String(text) => split_addresses(text).into_iter().next().unwrap_or_default(),
        Value::Array(items) => items.iter().find_map(|item| match item {
            Value::String(text) => split_addresses(text).into_iter().next(),
            Value::Object(_) => text_field(item, &["address", "number", "phoneNumber"]),
            _ => None,
        }).unwrap_or_default(),
        _ => String::new(),
    }
}

fn split_addresses(text: &str) -> Vec<String> {
    text.split([';', ','])
        .map(|part| part.trim().to_string())
        .filter(|part| !part.is_empty())
        .collect()
}

fn has_media(bean: &Value) -> bool {
    match bean.get("medias") {
        Some(Value::Array(items)) => !items.is_empty(),
        _ => false,
    }
}

fn value_id(value: &Value) -> String {
    match value {
        Value::String(text) => text.trim().to_string(),
        Value::Number(number) => number.to_string(),
        _ => String::new(),
    }
}

fn delete_ids(data: &Value) -> Vec<String> {
    let mut ids = Vec::new();
    for key in ["deleteList", "list", "msgIds", "ids"] {
        if let Some(Value::Array(items)) = data.get(key) {
            for item in items {
                match item {
                    Value::String(text) => ids.push(text.clone()),
                    Value::Number(number) => ids.push(number.to_string()),
                    Value::Object(_) => {
                        if let Some(id) = item.get("msgId").map(value_id) {
                            if !id.is_empty() {
                                ids.push(id);
                            }
                        }
                    }
                    _ => {}
                }
            }
        }
    }
    ids
}

fn last_batch(data: &Value) -> bool {
    let current = data.get("currentBatch").and_then(Value::as_i64).unwrap_or(1);
    let total = data.get("totalBatches").and_then(Value::as_i64).unwrap_or(1);
    total <= 0 || current >= total
}

fn send_failed(data: &Value) -> bool {
    if data.get("code").and_then(Value::as_i64).unwrap_or(0) != 0 {
        return true;
    }
    matches!(data.get("status").and_then(Value::as_i64), Some(status) if status < 0 || status == 2)
}

fn preview_of(item: &Item) -> String {
    let text = item.body.trim();
    if text.is_empty() {
        return "Message".into();
    }
    text.chars().take(80).collect()
}

fn now_ms() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|duration| duration.as_millis() as i64)
        .unwrap_or(0)
}

/// Drop picture payloads before the message is parsed. The panel only needs the text.
pub fn strip_media_data(input: &str) -> String {
    let bytes = input.as_bytes();
    let needle = b"\"mediaData\"";
    let mut out = Vec::with_capacity(input.len().min(65_536));
    let mut index = 0;
    while index < bytes.len() {
        if bytes[index..].starts_with(needle) {
            out.extend_from_slice(b"\"mediaData\"");
            index += needle.len();
            while index < bytes.len() && bytes[index].is_ascii_whitespace() {
                out.push(bytes[index]);
                index += 1;
            }
            if index < bytes.len() && bytes[index] == b':' {
                out.push(b':');
                index += 1;
            }
            while index < bytes.len() && bytes[index].is_ascii_whitespace() {
                out.push(bytes[index]);
                index += 1;
            }
            if index < bytes.len() && bytes[index] == b'"' {
                out.extend_from_slice(b"\"\"");
                index += 1;
                let mut escape = false;
                while index < bytes.len() {
                    let byte = bytes[index];
                    index += 1;
                    if escape {
                        escape = false;
                        continue;
                    }
                    if byte == b'\\' {
                        escape = true;
                        continue;
                    }
                    if byte == b'"' {
                        break;
                    }
                }
                continue;
            }
            continue;
        }
        out.push(bytes[index]);
        index += 1;
    }
    String::from_utf8(out).unwrap_or_else(|_| input.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn numbers_with_and_without_a_country_code_share_a_thread() {
        assert_eq!(thread_key("+91 98765 43210"), thread_key("9876543210"));
        assert_eq!(thread_key("+1 (415) 555-1212"), "4155551212");
        assert!(thread_key("Mom") != thread_key("Dad"));
    }

    #[test]
    fn a_disabled_phone_is_asked_once() {
        let mut sms = Sms::new();
        let first = sms.on_message(100, 1018, &json!({"granted": true, "enabled": false}));
        assert_eq!(first.sends.len(), 1);
        assert_eq!(first.sends[0]["cmd"], 18);
        assert_eq!(first.sends[0]["pId"], 110);
        assert!(first.sends[0].get("data").is_none());
        let second = sms.on_message(100, 1018, &json!({"granted": true, "enabled": false}));
        assert!(second.sends.is_empty());
        assert_eq!(sms.view().phase, "off");
    }

    #[test]
    fn an_enabled_phone_is_asked_for_texts() {
        let mut sms = Sms::new();
        let outcome = sms.on_message(100, 1018, &json!({"granted": true, "enabled": true}));
        assert!(outcome.sends.iter().any(|item| item["cmd"] == 28 && item["data"]["type"] == 1));
        assert!(outcome.notes.iter().all(|note| !note.contains("body") && note.len() < 80));
    }

    #[test]
    fn a_batch_groups_a_conversation_and_a_send_carries_no_media() {
        let mut sms = Sms::new();
        sms.on_message(100, 1018, &json!({"granted": true, "enabled": true}));
        sms.on_message(204, 2, &json!({"ver":"5.0","cmd":2,"pId":110,"data":[{"name": "Asha", "phone": ["+91 98765 43210"]}]}));
        sms.on_message(
            204,
            29,
            &json!({
                "ver": "5.0",
                "cmd": 29,
                "pId": 110,
                "data": {
                "type": 1,
                "currentBatch": 1,
                "totalBatches": 1,
                "messageBeanList": [{
                    "msgId": 44,
                    "messageType": 1,
                    "fromAddress": "+919876543210",
                    "toAddress": ["+15550001111"],
                    "body": "hello there",
                    "read": 0,
                    "date": 1_700_000_000_000i64,
                    "medias": [{"mediaData": "AAAA", "mimeType": "image/jpeg"}]
                }]
                }
            }),
        );
        let view = sms.view();
        assert_eq!(view.phase, "ready");
        assert_eq!(view.threads.len(), 1);
        assert_eq!(view.threads[0].title, "Asha");
        assert_eq!(view.threads[0].preview, "hello there");
        assert_eq!(view.unread, 1);
        let opened = sms.open(&view.threads[0].key).unwrap();
        assert_eq!(opened.sends[0]["cmd"], 8);
        assert_eq!(opened.sends[0]["data"]["status"], 1);
        let sent = sms.send_open("on my way").unwrap();
        let data = &sent.sends[0]["data"];
        assert_eq!(sent.sends[0]["pId"], 110);
        assert_eq!(data["medias"], json!([]));
        assert_eq!(data["messageType"], 2);
        assert_eq!(data["subId"], -1);
        assert_eq!(data["toAddress"][0], "+919876543210");
        assert!(sent.notes.is_empty());
        let shown = sms.view();
        assert_eq!(shown.open.len(), 2);
        assert!(shown.open.iter().any(|item| item.pending && item.outgoing));
    }

    #[test]
    fn media_bytes_are_removed_before_parsing() {
        let raw = r#"{"body":"hi","medias":[{"mediaData":"abc\"def","mimeType":"image/jpeg"}]}"#;
        let stripped = strip_media_data(raw);
        assert!(!stripped.contains("abc"));
        assert!(stripped.contains("\"body\":\"hi\""));
        let parsed: Value = serde_json::from_str(&stripped).unwrap();
        assert_eq!(parsed["medias"][0]["mediaData"], "");
    }

    #[test]
    fn notes_do_not_repeat_the_text() {
        let mut sms = Sms::new();
        sms.on_message(100, 1018, &json!({"granted": true, "enabled": true}));
        let secret = "private-sentence-should-not-leak";
        sms.on_message(
            204,
            29,
            &json!({
                "type": 1,
                "totalBatches": 1,
                "messageBeanList": [{
                    "msgId": 9,
                    "messageType": 1,
                    "fromAddress": "5551234567",
                    "body": secret,
                    "date": 10
                }]
            }),
        );
        let outcome = sms.on_message(
            204,
            7,
            &json!({
                "action": 1,
                "type": 1,
                "messageBeanList": [{
                    "msgId": 10,
                    "messageType": 1,
                    "fromAddress": "5551234567",
                    "body": secret,
                    "date": 11
                }]
            }),
        );
        assert!(outcome.notes.iter().all(|note| !note.contains(secret) && !note.contains("555")));
    }
}
