//! macOS banners posted by this app.
//!
//! `osascript` belongs to Script Editor, so those banners used Script Editor's
//! icon and opened Script Editor when clicked. These are delivered by Smart
//! Connect. The phone app's icon is the sender image, so the banner shows that
//! icon. If the phone icon is missing, Smart Connect's own icon is used.

use block2::RcBlock;
use dispatch2::DispatchQueue;
use objc2::rc::{Allocated, Retained};
use objc2::runtime::{AnyClass, AnyObject, NSObject, NSObjectProtocol, ProtocolObject};
use objc2::{define_class, msg_send, AnyThread};
use objc2_foundation::{NSData, NSError, NSString};
use objc2_user_notifications::{
    UNAuthorizationOptions, UNMutableNotificationContent, UNNotification, UNNotificationContent,
    UNNotificationContentProviding, UNNotificationPresentationOptions, UNNotificationRequest, UNNotificationResponse,
    UNNotificationSound, UNUserNotificationCenter, UNUserNotificationCenterDelegate,
};
use base64::Engine as _;
use serde::{Deserialize, Serialize};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex, Once};
use tauri::Manager;

#[link(name = "Intents", kind = "framework")]
unsafe extern "C" {}

const APP_ICON: &[u8] = include_bytes!("../icons/128x128.png");

// Badge | Sound | Alert.
const ASK: usize = (1 << 0) | (1 << 1) | (1 << 2);
// Sound | List | Banner. Alerts still show while this window is in front.
const PRESENT: usize = (1 << 1) | (1 << 3) | (1 << 4);

static NEXT_ID: AtomicU64 = AtomicU64::new(1);
static APP: Mutex<Option<tauri::AppHandle>> = Mutex::new(None);

#[derive(Clone, Serialize, Deserialize)]
struct BannerClick {
    key: String,
    pkg: String,
}

define_class!(
    #[unsafe(super(NSObject))]
    #[name = "SCNoticeDelegate"]
    struct NoticeDelegate;

    unsafe impl NSObjectProtocol for NoticeDelegate {}

    unsafe impl UNUserNotificationCenterDelegate for NoticeDelegate {
        #[unsafe(method(userNotificationCenter:willPresentNotification:withCompletionHandler:))]
        fn will_present(
            &self,
            _center: &UNUserNotificationCenter,
            _note: &UNNotification,
            handler: &block2::DynBlock<dyn Fn(UNNotificationPresentationOptions)>,
        ) {
            handler.call((UNNotificationPresentationOptions::from_bits_retain(PRESENT),));
        }

        #[unsafe(method(userNotificationCenter:didReceiveNotificationResponse:withCompletionHandler:))]
        fn did_receive(
            &self,
            _center: &UNUserNotificationCenter,
            response: &UNNotificationResponse,
            handler: &block2::DynBlock<dyn Fn()>,
        ) {
            focus_app();
            let identifier = response.notification().request().identifier().to_string();
            if let Some(payload) = decode_click(&identifier) {
                if let Some(app) = APP.lock().ok().and_then(|slot| slot.clone()) {
                    use tauri::Emitter;
                    let _ = app.emit("notification-banner-click", payload);
                }
            }
            handler.call(());
        }
    }
);

impl NoticeDelegate {
    fn new() -> Retained<Self> {
        let this = Self::alloc().set_ivars(());
        unsafe { msg_send![super(this), init] }
    }
}

pub fn bind_app(app: tauri::AppHandle) {
    if let Ok(mut slot) = APP.lock() {
        *slot = Some(app);
    }
}

/// Install the delegate so a click focuses this app. Safe to call more than once.
pub fn install() {
    if !crate::app_bundle::is_bundled() {
        return;
    }
    static ONCE: Once = Once::new();
    ONCE.call_once(|| {
        let installed = objc2::exception::catch(std::panic::AssertUnwindSafe(|| {
            let delegate = NoticeDelegate::new();
            let center = UNUserNotificationCenter::currentNotificationCenter();
            center.setDelegate(Some(ProtocolObject::from_ref(&*delegate)));
            // The center keeps only a weak reference.
            std::mem::forget(delegate);
        }));
        let _ = installed;
    });
}

/// `on_denied` runs only when macOS refuses banners for this app.
pub fn show(
    app: String,
    title: String,
    body: String,
    key: String,
    pkg: String,
    icon: Option<Vec<u8>>,
    on_denied: impl FnOnce() + Send + 'static,
) {
    let icon = icon.filter(|bytes| bytes.len() >= 32).unwrap_or_else(|| APP_ICON.to_vec());
    DispatchQueue::main().exec_async(move || deliver(app, title, body, key, pkg, icon, on_denied));
}

fn deliver(
    app: String,
    title: String,
    body: String,
    key: String,
    pkg: String,
    icon: Vec<u8>,
    on_denied: impl FnOnce() + Send + 'static,
) {
    if !crate::app_bundle::is_bundled() {
        on_denied();
        return;
    }
    install();
    let on_denied = Arc::new(Mutex::new(Some(on_denied)));
    let payload = Arc::new(Mutex::new(Some((app, title, body, key, pkg, icon))));
    let denied_for_block = Arc::clone(&on_denied);
    let payload_for_block = Arc::clone(&payload);
    let done = RcBlock::new(move |granted: objc2::runtime::Bool, _error: *mut NSError| {
        let granted = granted.as_bool();
        let on_denied = Arc::clone(&denied_for_block);
        let payload = Arc::clone(&payload_for_block);
        DispatchQueue::main().exec_async(move || {
            if !granted {
                if let Some(callback) = on_denied.lock().ok().and_then(|mut slot| slot.take()) {
                    callback();
                }
                return;
            }
            let Some((app, title, body, key, pkg, icon)) = payload.lock().ok().and_then(|mut slot| slot.take()) else {
                return;
            };
            post(&UNUserNotificationCenter::currentNotificationCenter(), &app, &title, &body, &key, &pkg, &icon);
        });
    });
    let center = UNUserNotificationCenter::currentNotificationCenter();
    center.requestAuthorizationWithOptions_completionHandler(UNAuthorizationOptions::from_bits_retain(ASK), &*done);
}

fn post(center: &UNUserNotificationCenter, app: &str, title: &str, body: &str, key: &str, pkg: &str, icon: &[u8]) {
    let content = UNMutableNotificationContent::new();
    let heading = if title.is_empty() { app } else { title };
    content.setTitle(&NSString::from_str(heading));
    if !app.is_empty() && app != heading {
        content.setSubtitle(&NSString::from_str(app));
    }
    content.setBody(&NSString::from_str(if body.is_empty() { " " } else { body }));
    content.setSound(Some(&UNNotificationSound::defaultSound()));

    let delivered = sender_icon(&content, app, icon).unwrap_or_else(|| Retained::into_super(content));
    let id = NEXT_ID.fetch_add(1, Ordering::Relaxed);
    let click = BannerClick { key: key.to_string(), pkg: pkg.to_string() };
    let encoded = base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(serde_json::to_vec(&click).unwrap_or_default());
    let request = UNNotificationRequest::requestWithIdentifier_content_trigger(
        &NSString::from_str(&format!("sc-{id}-{encoded}")),
        &delivered,
        None,
    );
    center.addNotificationRequest_withCompletionHandler(&request, None);
}

fn decode_click(identifier: &str) -> Option<BannerClick> {
    let encoded = identifier.splitn(3, '-').nth(2)?;
    let bytes = base64::engine::general_purpose::URL_SAFE_NO_PAD.decode(encoded).ok()?;
    serde_json::from_slice(&bytes).ok()
}

fn focus_app() {
    if let Some(cls) = AnyClass::get(c"NSApplication") {
        unsafe {
            let app: Option<Retained<AnyObject>> = msg_send![cls, sharedApplication];
            if let Some(app) = app {
                let _: () = msg_send![&*app, activateIgnoringOtherApps: true];
            }
        }
    }
    let Some(app) = APP.lock().ok().and_then(|slot| slot.clone()) else {
        return;
    };
    if let Some(window) = app.get_webview_window("main") {
        let _ = window.unminimize();
        let _ = window.show();
        let _ = window.set_focus();
    }
}

/// Ask macOS to draw this image in the banner's icon slot.
fn sender_icon(content: &UNMutableNotificationContent, app: &str, png: &[u8]) -> Option<Retained<UNNotificationContent>> {
    let image_cls = AnyClass::get(c"INImage")?;
    let handle_cls = AnyClass::get(c"INPersonHandle")?;
    let person_cls = AnyClass::get(c"INPerson")?;
    let intent_cls = AnyClass::get(c"INSendMessageIntent")?;
    let data = NSData::with_bytes(png);
    let name = NSString::from_str(if app.is_empty() { "Phone" } else { app });
    unsafe {
        let image: Option<Retained<AnyObject>> = msg_send![image_cls, imageWithImageData: &*data];
        let image = image?;
        let allocated: Allocated<AnyObject> = msg_send![handle_cls, alloc];
        let handle: Option<Retained<AnyObject>> = msg_send![allocated, initWithValue: &*name, type: 0isize];
        let handle = handle?;
        let allocated: Allocated<AnyObject> = msg_send![person_cls, alloc];
        let person: Option<Retained<AnyObject>> = msg_send![
            allocated,
            initWithPersonHandle: &*handle,
            nameComponents: None::<&AnyObject>,
            displayName: &*name,
            image: &*image,
            contactIdentifier: None::<&AnyObject>,
            customIdentifier: &*name,
        ];
        let person = person?;
        let allocated: Allocated<AnyObject> = msg_send![intent_cls, alloc];
        let intent: Option<Retained<AnyObject>> = msg_send![
            allocated,
            initWithRecipients: None::<&AnyObject>,
            outgoingMessageType: 1isize,
            content: &*NSString::from_str(""),
            speakableGroupName: None::<&AnyObject>,
            conversationIdentifier: &*name,
            serviceName: &*name,
            sender: &*person,
            attachments: None::<&AnyObject>,
        ];
        let intent = intent?;
        let provider: Retained<ProtocolObject<dyn UNNotificationContentProviding>> = Retained::cast_unchecked(intent);
        content.contentByUpdatingWithProvider_error(&provider).ok()
    }
}
