//! Wake a paired phone whose Smart Connect app is in the background.
//!
//! The phone advertises ReadyFor service `0x5246`. The last 16 bytes of that
//! service data are its session nonce. Writing the sealed invite to
//! characteristic `0x0722` asks the phone to dial this Mac. The radio is never
//! powered on: if Bluetooth is off, the scan is skipped.

use std::cell::UnsafeCell;
use std::collections::HashMap;
use std::time::Duration;

use dispatch2::DispatchQueue;
use objc2::rc::{Allocated, Retained};
use objc2::runtime::{AnyClass, AnyObject};
use objc2::{define_class, msg_send, AnyThread, Message};
use objc2_foundation::{NSData, NSError, NSObject, NSObjectProtocol, NSString};
use tokio::sync::mpsc::{unbounded_channel, UnboundedSender};
use uuid::Uuid;

use crate::protocol;

/// Bluetooth base UUID with the 16-bit ReadyFor service id.
pub const READYFOR: Uuid = Uuid::from_u128(0x0000_5246_0000_1000_8000_0080_5f9b_34fb);
const READYFOR_TEXT: &str = "00005246-0000-1000-8000-00805F9B34FB";
const INVITE_TEXT: &str = "00000722-0000-1000-8000-00805F9B34FB";
const NOTIFY_TEXT: &str = "00000726-0000-1000-8000-00805F9B34FB";

const SCAN_FOR: Duration = Duration::from_secs(30);
/// How long to leave the radio up while the Bluetooth permission dialog is on screen.
const PERMISSION_WAIT: Duration = Duration::from_secs(60);
/// `CBManagerStatePoweredOn`.
const POWERED_ON: isize = 5;
/// `CBCharacteristicWriteWithResponse`.
const WRITE_WITH_RESPONSE: isize = 0;

#[link(name = "CoreBluetooth", kind = "framework")]
unsafe extern "C" {
    static CBAdvertisementDataServiceDataKey: *const AnyObject;
    static CBCentralManagerScanOptionAllowDuplicatesKey: *const AnyObject;
    static CBCentralManagerOptionShowPowerAlertKey: *const AnyObject;
}

pub struct Target {
    pub name: String,
    pub real: Vec<u8>,
    pub frame: Vec<u8>,
}

/// Counts from one scan. Safe to share with the worker thread.
#[derive(Default)]
pub struct WakeStats {
    pub adverts: std::sync::atomic::AtomicU32,
    pub payloads: std::sync::atomic::AtomicU32,
    pub matched: std::sync::atomic::AtomicU32,
}

pub struct WakeOutcome {
    pub names: Vec<String>,
    pub adverts: u32,
    pub payloads: u32,
    pub matched: u32,
}

enum WakeMsg {
    Woken(String),
    Failed(String),
    /// Radio is on. The scan window starts now, after any permission dialog.
    Scanning,
}

struct Session {
    tx: UnboundedSender<WakeMsg>,
    targets: Vec<Target>,
    delegate: Retained<WakeDelegate>,
    manager: Retained<AnyObject>,
    service_uuid: Retained<AnyObject>,
    invite_uuid: Retained<AnyObject>,
    notify_uuid: Retained<AnyObject>,
    peripheral: Option<Retained<AnyObject>>,
    active_name: String,
    active_frame: Vec<u8>,
    scanning: bool,
    wrote: bool,
    seen: Vec<String>,
    stats: std::sync::Arc<WakeStats>,
}

struct MainOnly(UnsafeCell<Option<Session>>);
// Touched only on the main queue. The worker thread only schedules work there.
unsafe impl Sync for MainOnly {}

static SESSION: MainOnly = MainOnly(UnsafeCell::new(None));

fn slot() -> &'static mut Option<Session> {
    unsafe { &mut *SESSION.0.get() }
}

define_class!(
    #[unsafe(super(NSObject))]
    #[name = "SCWakeDelegate"]
    struct WakeDelegate;

    unsafe impl NSObjectProtocol for WakeDelegate {}

    impl WakeDelegate {
        #[unsafe(method(centralManagerDidUpdateState:))]
        fn did_update_state(&self, central: &AnyObject) {
            let state: isize = unsafe { msg_send![central, state] };
            on_state(state);
        }

        #[unsafe(method(centralManager:didDiscoverPeripheral:advertisementData:RSSI:))]
        fn did_discover(
            &self,
            _central: &AnyObject,
            peripheral: &AnyObject,
            advert: &AnyObject,
            _rssi: &AnyObject,
        ) {
            on_discover(peripheral, advert);
        }

        #[unsafe(method(centralManager:didConnectPeripheral:))]
        fn did_connect(&self, _central: &AnyObject, peripheral: &AnyObject) {
            on_connected(peripheral);
        }

        #[unsafe(method(centralManager:didFailToConnectPeripheral:error:))]
        fn did_fail(&self, _central: &AnyObject, peripheral: &AnyObject, _error: Option<&NSError>) {
            give_up_peripheral(peripheral);
        }

        #[unsafe(method(peripheral:didDiscoverServices:))]
        fn did_services(&self, peripheral: &AnyObject, error: Option<&NSError>) {
            if error.is_some() {
                give_up_peripheral(peripheral);
                return;
            }
            discover_characteristics(peripheral);
        }

        #[unsafe(method(peripheral:didDiscoverCharacteristicsForService:error:))]
        fn did_characteristics(&self, peripheral: &AnyObject, service: &AnyObject, error: Option<&NSError>) {
            if error.is_some() {
                give_up_peripheral(peripheral);
                return;
            }
            write_invite(peripheral, service);
        }

        #[unsafe(method(peripheral:didWriteValueForCharacteristic:error:))]
        fn did_write(&self, peripheral: &AnyObject, characteristic: &AnyObject, error: Option<&NSError>) {
            on_wrote(peripheral, characteristic, error.is_some());
        }

        #[unsafe(method(peripheral:didUpdateValueForCharacteristic:error:))]
        fn did_value(&self, _peripheral: &AnyObject, characteristic: &AnyObject, error: Option<&NSError>) {
            let uuid = uuid_text(characteristic);
            let value: Option<Retained<NSData>> = unsafe { msg_send![characteristic, value] };
            let len = value.as_ref().map(|data| data.len()).unwrap_or(0);
            eprintln!("wake: notify uuid={uuid} len={len} err={}", error.is_some());
        }
    }
);

impl WakeDelegate {
    fn new() -> Retained<Self> {
        let this = Self::alloc().set_ivars(());
        unsafe { msg_send![super(this), init] }
    }
}

/// Last 16 bytes of the ReadyFor service data, when the advert is long enough.
pub fn nonce_from_service_data(data: &HashMap<Uuid, Vec<u8>>) -> Option<Vec<u8>> {
    let bytes = data.get(&READYFOR)?;
    if bytes.len() <= 15 {
        return None;
    }
    Some(bytes[bytes.len() - 16..].to_vec())
}

pub fn matching<'a>(nonce: &[u8], targets: &'a [Target]) -> Option<&'a Target> {
    targets.iter().find(|target| protocol::nonce_matches(nonce, &target.real))
}

/// Scan until a saved phone accepts the invite, or the scan window ends.
/// Names are the phones that accepted the write.
pub async fn push_invites(targets: Vec<Target>) -> Result<WakeOutcome, String> {
    if targets.is_empty() {
        return Ok(WakeOutcome { names: Vec::new(), adverts: 0, payloads: 0, matched: 0 });
    }
    let (tx, mut rx) = unbounded_channel();
    let stats = std::sync::Arc::new(WakeStats::default());
    let stats_main = stats.clone();
    DispatchQueue::main().exec_async(move || start_on_main(targets, tx, stats_main));

    let mut names = Vec::new();
    let mut failed = None;
    let mut deadline = tokio::time::Instant::now() + PERMISSION_WAIT;
    let mut scan_armed = false;
    loop {
        let left = deadline.saturating_duration_since(tokio::time::Instant::now());
        if left.is_zero() {
            break;
        }
        tokio::select! {
            _ = tokio::time::sleep(left) => break,
            msg = rx.recv() => match msg {
                Some(WakeMsg::Scanning) => {
                    if !scan_armed {
                        scan_armed = true;
                        deadline = tokio::time::Instant::now() + SCAN_FOR;
                    }
                }
                Some(WakeMsg::Woken(name)) => names.push(name),
                Some(WakeMsg::Failed(err)) => {
                    failed = Some(err);
                    break;
                }
                None => break,
            },
        }
    }
    DispatchQueue::main().exec_async(stop_on_main);
    let outcome = WakeOutcome {
        names,
        adverts: stats.adverts.load(std::sync::atomic::Ordering::Relaxed),
        payloads: stats.payloads.load(std::sync::atomic::Ordering::Relaxed),
        matched: stats.matched.load(std::sync::atomic::Ordering::Relaxed),
    };
    eprintln!(
        "wake: adverts={} payloads={} matched={}",
        outcome.adverts, outcome.payloads, outcome.matched
    );
    if outcome.names.is_empty() {
        if let Some(err) = failed {
            return Err(err);
        }
    }
    Ok(outcome)
}

fn start_on_main(targets: Vec<Target>, tx: UnboundedSender<WakeMsg>, stats: std::sync::Arc<WakeStats>) {
    stop_on_main();
    let Ok(delegate) = objc2::exception::catch(std::panic::AssertUnwindSafe(WakeDelegate::new)) else {
        let _ = tx.send(WakeMsg::Failed(asleep()));
        return;
    };
    let Some(manager) = manager_with(&delegate) else {
        let _ = tx.send(WakeMsg::Failed("Bluetooth is not available on this Mac.".into()));
        return;
    };
    let Some(service_uuid) = cbuuid(READYFOR_TEXT) else {
        let _ = tx.send(WakeMsg::Failed("Bluetooth is not available on this Mac.".into()));
        return;
    };
    let (Some(invite_uuid), Some(notify_uuid)) = (cbuuid(INVITE_TEXT), cbuuid(NOTIFY_TEXT)) else {
        let _ = tx.send(WakeMsg::Failed("Bluetooth is not available on this Mac.".into()));
        return;
    };
    let state: isize = unsafe { msg_send![&*manager, state] };
    *slot() = Some(Session {
            tx,
            targets,
            delegate,
            manager,
            service_uuid,
            invite_uuid,
            notify_uuid,
            peripheral: None,
            active_name: String::new(),
            active_frame: Vec::new(),
            scanning: false,
            wrote: false,
            seen: Vec::new(),
            stats,
        });
    on_state(state);
}

fn manager_with(delegate: &WakeDelegate) -> Option<Retained<AnyObject>> {
    let class = AnyClass::get(c"CBCentralManager")?;
    let allocated: Allocated<AnyObject> = unsafe { msg_send![class, alloc] };
    // Default options ask the user to turn Bluetooth on. Leave the radio as it is.
    let options = bool_option(unsafe { CBCentralManagerOptionShowPowerAlertKey }, false);
    let manager: Retained<AnyObject> = match &options {
        Some(options) => unsafe {
            msg_send![
                allocated,
                initWithDelegate: delegate,
                queue: std::ptr::null::<AnyObject>(),
                options: &**options
            ]
        },
        None => unsafe {
            msg_send![
                allocated,
                initWithDelegate: delegate,
                queue: std::ptr::null::<AnyObject>()
            ]
        },
    };
    Some(manager)
}

fn bool_option(key_ptr: *const AnyObject, on: bool) -> Option<Retained<AnyObject>> {
    if key_ptr.is_null() {
        return None;
    }
    let key = unsafe { &*key_ptr };
    let number = AnyClass::get(c"NSNumber")?;
    let value: Retained<AnyObject> = unsafe { msg_send![number, numberWithBool: on] };
    let dict = AnyClass::get(c"NSDictionary")?;
    let options: Retained<AnyObject> = unsafe { msg_send![dict, dictionaryWithObject: &*value, forKey: key] };
    Some(options)
}

fn cbuuid(text: &str) -> Option<Retained<AnyObject>> {
    let class = AnyClass::get(c"CBUUID")?;
    let name = NSString::from_str(text);
    let uuid: Retained<AnyObject> = unsafe { msg_send![class, UUIDWithString: &*name] };
    Some(uuid)
}

fn on_state(state: isize) {
    if state == POWERED_ON {
        begin_scan();
        if let Some(session) = slot().as_ref() {
            let _ = session.tx.send(WakeMsg::Scanning);
        }
        return;
    }
    // 0 unknown and 1 resetting can still become powered on.
    if state == 0 || state == 1 {
        return;
    }
    let err = if state == 3 {
        "Bluetooth permission is off for Smart Connect. Allow it in System Settings, or open Smart Connect on the phone.".into()
    } else {
        asleep()
    };
    end(WakeMsg::Failed(err));
}

fn begin_scan() {
    let Some(session) = slot().as_mut() else { return };
    if session.scanning || session.peripheral.is_some() {
        return;
    }
    let class = AnyClass::get(c"NSArray");
    let Some(class) = class else { return };
    let services: Retained<AnyObject> = unsafe { msg_send![class, arrayWithObject: &*session.service_uuid] };
    let manager = session.manager.clone();
    session.scanning = true;
    // The wake nonce is often only in the scan response. Without duplicates,
    // CoreBluetooth delivers the first packet and hides that response.
    let options = bool_option(unsafe { CBCentralManagerScanOptionAllowDuplicatesKey }, true);
    unsafe {
        let _: () = match &options {
            Some(options) => msg_send![&*manager, scanForPeripheralsWithServices: &*services, options: &**options],
            None => msg_send![&*manager, scanForPeripheralsWithServices: &*services, options: std::ptr::null::<AnyObject>()],
        };
    }
}

fn on_discover(peripheral: &AnyObject, advert: &AnyObject) {
    if let Some(session) = slot().as_ref() {
        session.stats.adverts.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    }
    let Some(bytes) = service_payload(advert) else { return };
    if bytes.len() <= 15 {
        return;
    }
    let nonce = &bytes[bytes.len() - 16..];
    let ident = peripheral_id(peripheral);
    let Some(session) = slot().as_mut() else { return };
    session.stats.payloads.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    if session.seen.iter().any(|seen| seen == &ident) || session.peripheral.is_some() {
        return;
    }
    let Some(index) = session.targets.iter().position(|target| protocol::nonce_matches(nonce, &target.real)) else {
        return;
    };
    session.stats.matched.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    let target = session.targets.remove(index);
    session.active_name = target.name;
    session.active_frame = target.frame;
    session.scanning = false;
    session.peripheral = Some(peripheral.retain());
    let manager = session.manager.clone();
    let delegate = session.delegate.clone();
        unsafe {
        let _: () = msg_send![&*manager, stopScan];
        let _: () = msg_send![peripheral, setDelegate: &*delegate];
        let _: () = msg_send![&*manager, connectPeripheral: peripheral, options: std::ptr::null::<AnyObject>()];
    }
}

fn on_connected(peripheral: &AnyObject) {
    let Some(session) = slot().as_ref() else { return };
    let service = session.service_uuid.clone();
        let class = AnyClass::get(c"NSArray");
    let Some(class) = class else { return };
    let services: Retained<AnyObject> = unsafe { msg_send![class, arrayWithObject: &*service] };
    unsafe {
        let _: () = msg_send![peripheral, discoverServices: &*services];
    }
}

fn discover_characteristics(peripheral: &AnyObject) {
    let services: Option<Retained<AnyObject>> = unsafe { msg_send![peripheral, services] };
    let Some(services) = services else { return };
    let count: usize = unsafe { msg_send![&*services, count] };
    for index in 0..count {
        let service: Retained<AnyObject> = unsafe { msg_send![&*services, objectAtIndex: index] };
        unsafe {
            let _: () = msg_send![
                peripheral,
                discoverCharacteristics: std::ptr::null::<AnyObject>(),
                forService: &*service
            ];
        }
    }
}

fn write_invite(peripheral: &AnyObject, service: &AnyObject) {
    let Some(session) = slot().as_mut() else { return };
    if session.wrote {
        return;
    }
    let characteristics: Option<Retained<AnyObject>> = unsafe { msg_send![service, characteristics] };
    let Some(characteristics) = characteristics else { return };
    let count: usize = unsafe { msg_send![&*characteristics, count] };
    let mut invite: Option<Retained<AnyObject>> = None;
    for index in 0..count {
        let characteristic: Retained<AnyObject> = unsafe { msg_send![&*characteristics, objectAtIndex: index] };
        let uuid: Option<Retained<AnyObject>> = unsafe { msg_send![&*characteristic, UUID] };
        let Some(uuid) = uuid else { continue };
        let is_invite: bool = unsafe { msg_send![&*uuid, isEqual: &*session.invite_uuid] };
        let is_notify: bool = unsafe { msg_send![&*uuid, isEqual: &*session.notify_uuid] };
        if is_notify || is_invite {
            unsafe {
                let _: () = msg_send![peripheral, setNotifyValue: true, forCharacteristic: &*characteristic];
            }
        }
        if is_invite {
            invite = Some(characteristic);
        }
    }
    let Some(invite) = invite else {
        eprintln!("wake: ReadyFor service has no invite characteristic");
        return;
    };
    session.wrote = true;
    let len = session.active_frame.len();
    eprintln!("wake: write bytes={len} uuid={}", uuid_text(&invite));
    let data = NSData::with_bytes(&session.active_frame);
    unsafe {
        let _: () = msg_send![
            peripheral,
            writeValue: &*data,
            forCharacteristic: &*invite,
            type: WRITE_WITH_RESPONSE
        ];
    }
}

fn on_wrote(peripheral: &AnyObject, characteristic: &AnyObject, failed: bool) {
    let name = {
        let Some(session) = slot().as_ref() else { return };
        let uuid: Option<Retained<AnyObject>> = unsafe { msg_send![characteristic, UUID] };
        let is_invite = uuid.is_some_and(|uuid| unsafe { msg_send![&*uuid, isEqual: &*session.invite_uuid] });
        if !is_invite {
            return;
        }
        session.active_name.clone()
    };
    if failed {
        eprintln!("wake: write failed");
        give_up_peripheral(peripheral);
        return;
    }
    // Leave the radio up. The phone dials back over Wi-Fi while this link is still open.
    eprintln!("wake: write accepted");
    if let Some(session) = slot().as_mut() {
        let _ = session.tx.send(WakeMsg::Woken(name));
        session.wrote = true;
    }
}

fn uuid_text(object: &AnyObject) -> String {
    let uuid: Option<Retained<AnyObject>> = unsafe { msg_send![object, UUID] };
    let Some(uuid) = uuid else { return String::new() };
    let text: Option<Retained<NSString>> = unsafe { msg_send![&*uuid, UUIDString] };
    text.map(|text| text.to_string()).unwrap_or_default()
}

fn give_up_peripheral(peripheral: &AnyObject) {
    let ident = peripheral_id(peripheral);
    let Some(session) = slot().as_mut() else { return };
    session.seen.push(ident);
    session.peripheral = None;
    session.active_frame.clear();
    session.scanning = false;
    session.wrote = false;
    let manager = session.manager.clone();
    let resume = !session.targets.is_empty();
        unsafe {
        let _: () = msg_send![&*manager, cancelPeripheralConnection: peripheral];
    }
    if resume {
        begin_scan();
    }
}

fn peripheral_id(peripheral: &AnyObject) -> String {
    let ident: Option<Retained<AnyObject>> = unsafe { msg_send![peripheral, identifier] };
    let Some(ident) = ident else { return String::new() };
    let text: Option<Retained<NSString>> = unsafe { msg_send![&*ident, UUIDString] };
    text.map(|text| text.to_string()).unwrap_or_default()
}

fn service_payload(advert: &AnyObject) -> Option<Vec<u8>> {
    let key_ptr = unsafe { CBAdvertisementDataServiceDataKey };
    if key_ptr.is_null() {
        return None;
    }
    let key = unsafe { &*key_ptr };
    let services: Option<Retained<AnyObject>> = unsafe { msg_send![advert, objectForKey: key] };
    let services = services?;
    let ready = cbuuid(READYFOR_TEXT)?;
    let keys: Retained<AnyObject> = unsafe { msg_send![&*services, allKeys] };
    let count: usize = unsafe { msg_send![&*keys, count] };
    for index in 0..count {
        let uuid: Retained<AnyObject> = unsafe { msg_send![&*keys, objectAtIndex: index] };
        // A 16-bit UUID's string is "5246", not the 128-bit form we scan for.
        // CBUUID equality treats those as the same service.
        let same: bool = unsafe { msg_send![&*uuid, isEqual: &*ready] };
        if !same && !uuid_text_is_readyfor(&uuid) {
            continue;
        }
        let data: Option<Retained<NSData>> = unsafe { msg_send![&*services, objectForKey: &*uuid] };
        return data.map(|data| data.to_vec());
    }
    None
}

fn uuid_text_is_readyfor(uuid: &AnyObject) -> bool {
    let text: Option<Retained<NSString>> = unsafe { msg_send![uuid, UUIDString] };
    let Some(text) = text else { return false };
    let text = text.to_string();
    let long = if text.len() == 4 {
        format!("0000{text}-0000-1000-8000-00805F9B34FB")
    } else if text.len() == 8 {
        format!("{text}-0000-1000-8000-00805F9B34FB")
    } else {
        text
    };
    long.eq_ignore_ascii_case(READYFOR_TEXT)
}

/// `Retained` is not `Send`. The drop is queued as a pointer and runs on the
/// main queue after the delegate callback that asked for it has returned.
struct Later(*mut Session);
unsafe impl Send for Later {}

impl Later {
    fn release(self) {
        unsafe { drop(Box::from_raw(self.0)) }
    }
}

fn defer(session: Session) {
    let later = Later(Box::into_raw(Box::new(session)));
    DispatchQueue::main().exec_async(move || later.release());
}

fn end(msg: WakeMsg) {
    let session = slot().take();
    let Some(session) = session else { return };
    halt(&session);
    let _ = session.tx.send(msg);
    defer(session);
}

fn end_quiet() {
    let session = slot().take();
    let Some(session) = session else { return };
    halt(&session);
    defer(session);
}

fn stop_on_main() {
    let session = slot().take();
    let Some(session) = session else { return };
    halt(&session);
    defer(session);
}

fn halt(session: &Session) {
    unsafe {
        let _: () = msg_send![&*session.manager, stopScan];
        if let Some(peripheral) = &session.peripheral {
            let _: () = msg_send![&*session.manager, cancelPeripheralConnection: &**peripheral];
        }
    }
}

fn asleep() -> String {
    "Bluetooth is off, so a backgrounded phone stays asleep. Open Smart Connect on the phone, or turn Bluetooth on.".into()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn service_data_uses_the_last_sixteen_bytes() {
        let real = b"phone-stable-id-bytes";
        let nonce = protocol::fresh_nonce(real);
        let mut blob = vec![1, 2, 3, 4];
        blob.extend_from_slice(&nonce);
        let mut data = HashMap::new();
        data.insert(READYFOR, blob);
        let found = nonce_from_service_data(&data).expect("nonce");
        assert!(protocol::nonce_matches(&found, real));
        assert!(matching(
            &found,
            &[Target {
                name: "Edge".into(),
                real: real.to_vec(),
                frame: vec![9],
            }]
        )
        .is_some());
    }

    #[test]
    fn a_short_advert_is_ignored() {
        let mut data = HashMap::new();
        data.insert(READYFOR, vec![0; 15]);
        assert!(nonce_from_service_data(&data).is_none());
    }
}
