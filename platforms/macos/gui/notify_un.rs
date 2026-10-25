//! Notifications through `UNUserNotificationCenter`; works only for a process inside an `.app` bundle.
//!
//! `show` is called from the `tray-notify` worker thread and blocks it: every asynchronous answer of the center is awaited
//! on a channel for a bounded time, so a stuck permission prompt cannot hang the worker forever.

use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::mpsc::{channel, Receiver};
use std::sync::OnceLock;
use std::time::Duration;

use block2::RcBlock;
use objc2::rc::Retained;
use objc2::runtime::{Bool, NSObject, NSObjectProtocol, ProtocolObject};
use objc2::{define_class, msg_send, AnyThread};
use objc2_foundation::{NSError, NSString};
use objc2_user_notifications::{
    UNAuthorizationOptions, UNMutableNotificationContent, UNNotification,
    UNNotificationPresentationOptions, UNNotificationRequest, UNNotificationSound,
    UNUserNotificationCenter, UNUserNotificationCenterDelegate,
};

use meshtown_core::gui::tray::REASON_NOTIFY_FAILED;

/// How long to wait for the delivery of a request.
const ANSWER_TIMEOUT: Duration = Duration::from_secs(3);
/// How long to wait for the permission prompt: a person has to answer it.
const PROMPT_TIMEOUT: Duration = Duration::from_secs(30);

static NEXT_ID: AtomicU64 = AtomicU64::new(1);
static DELEGATE: OnceLock<DelegateHandle> = OnceLock::new();

define_class!(
    // SAFETY:
    // - The superclass NSObject requires nothing from subclasses.
    // - `CenterDelegate` has no ivars and does not implement `Drop`.
    #[unsafe(super = NSObject)]
    struct CenterDelegate;

    // SAFETY: `NSObjectProtocol` has no safety requirements.
    unsafe impl NSObjectProtocol for CenterDelegate {}

    // SAFETY: the method signature matches the protocol.
    unsafe impl UNUserNotificationCenterDelegate for CenterDelegate {
        #[unsafe(method(userNotificationCenter:willPresentNotification:withCompletionHandler:))]
        fn will_present(
            &self,
            _center: &UNUserNotificationCenter,
            _notification: &UNNotification,
            completion_handler: &block2::DynBlock<dyn Fn(UNNotificationPresentationOptions)>,
        ) {
            // Show the banner even while the app is frontmost.
            let options = UNNotificationPresentationOptions::Banner
                | UNNotificationPresentationOptions::List
                | UNNotificationPresentationOptions::Sound;
            completion_handler.call((options,));
        }
    }
);

/// Keeps the delegate alive for the whole process (the center holds it weakly).
struct DelegateHandle(#[allow(dead_code)] Retained<CenterDelegate>);

// SAFETY: the delegate has no ivars and no mutable state, and its only method just calls the handler it is given; the
// handle is only stored, never used for messaging from Rust, so sharing it between threads cannot race.
unsafe impl Send for DelegateHandle {}
// SAFETY: see above.
unsafe impl Sync for DelegateHandle {}

fn install_delegate(center: &UNUserNotificationCenter) {
    let handle = DELEGATE.get_or_init(|| {
        let this = CenterDelegate::alloc().set_ivars(());
        // SAFETY: the `init` signature of `NSObject` is correct.
        DelegateHandle(unsafe { msg_send![super(this), init] })
    });
    center.setDelegate(Some(ProtocolObject::from_ref(&*handle.0)));
}

/// Waits for the answer of an asynchronous call; a timeout is a failure.
fn wait<T>(rx: &Receiver<T>, limit: Duration) -> Result<T, &'static str> {
    rx.recv_timeout(limit).map_err(|_| REASON_NOTIFY_FAILED)
}

fn authorize(center: &UNUserNotificationCenter) -> Result<(), &'static str> {
    let (tx, rx) = channel();
    let block = RcBlock::new(move |granted: Bool, _error: *mut NSError| {
        let _ = tx.send(granted.is_true());
    });
    center.requestAuthorizationWithOptions_completionHandler(
        UNAuthorizationOptions::Alert | UNAuthorizationOptions::Sound,
        &block,
    );
    if wait(&rx, PROMPT_TIMEOUT)? {
        Ok(())
    } else {
        Err(REASON_NOTIFY_FAILED)
    }
}

fn deliver(center: &UNUserNotificationCenter, title: &str, body: &str) -> Result<(), &'static str> {
    let content = UNMutableNotificationContent::new();
    content.setTitle(&NSString::from_str(title));
    content.setBody(&NSString::from_str(body));
    content.setSound(Some(&UNNotificationSound::defaultSound()));
    let id = NSString::from_str(&format!(
        "meshtown-{}",
        NEXT_ID.fetch_add(1, Ordering::Relaxed)
    ));
    let request = UNNotificationRequest::requestWithIdentifier_content_trigger(&id, &content, None);

    let (tx, rx) = channel();
    let block = RcBlock::new(move |error: *mut NSError| {
        let _ = tx.send(error.is_null());
    });
    center.addNotificationRequest_withCompletionHandler(&request, Some(&block));
    if wait(&rx, ANSWER_TIMEOUT)? {
        Ok(())
    } else {
        Err(REASON_NOTIFY_FAILED)
    }
}

/// Shows a banner through the notification center; blocks until the center has accepted it.
pub(crate) fn show(title: &str, body: &str) -> Result<(), &'static str> {
    let center = UNUserNotificationCenter::currentNotificationCenter();
    install_delegate(&center);
    authorize(&center)?;
    deliver(&center, title, body)
}
