use anyhow::Result;
use core_foundation::base::TCFType;
use core_foundation::boolean::CFBoolean;
use core_foundation::dictionary::CFDictionary;
use core_foundation::string::CFString;
use objc::{class, msg_send, sel, sel_impl};

// Link AVFoundation for AVCaptureDevice microphone permission APIs
#[link(name = "AVFoundation", kind = "framework")]
extern "C" {}

/// Microphone authorization status (mirrors AVAuthorizationStatus)
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MicrophoneAuthStatus {
    /// User has not yet been asked
    NotDetermined = 0,
    /// Restricted by parental controls / MDM
    Restricted = 1,
    /// User explicitly denied
    Denied = 2,
    /// User granted access
    Authorized = 3,
}

/// Check the current microphone authorization status via AVCaptureDevice.
pub fn microphone_authorization_status() -> MicrophoneAuthStatus {
    unsafe {
        let cls = match objc::runtime::Class::get("AVCaptureDevice") {
            Some(c) => c,
            None => {
                log::error!("AVCaptureDevice class not found");
                return MicrophoneAuthStatus::NotDetermined;
            }
        };

        // AVMediaTypeAudio is the NSString @"soun"
        let media_type: *mut objc::runtime::Object =
            msg_send![class!(NSString), stringWithUTF8String: c"soun".as_ptr()];

        let status: i64 = msg_send![cls, authorizationStatusForMediaType: media_type];

        match status {
            0 => MicrophoneAuthStatus::NotDetermined,
            1 => MicrophoneAuthStatus::Restricted,
            2 => MicrophoneAuthStatus::Denied,
            3 => MicrophoneAuthStatus::Authorized,
            _ => {
                log::warn!("Unknown AVAuthorizationStatus: {}", status);
                MicrophoneAuthStatus::NotDetermined
            }
        }
    }
}

/// Request microphone access, triggering the macOS permission dialog if needed.
/// Blocks until the user responds. Returns `true` if access was granted.
pub fn request_microphone_access() -> bool {
    let status = microphone_authorization_status();

    // Already determined — don't re-prompt
    if status == MicrophoneAuthStatus::Authorized {
        return true;
    }
    if status == MicrophoneAuthStatus::Denied || status == MicrophoneAuthStatus::Restricted {
        return false;
    }

    // Status is NotDetermined — trigger the OS dialog
    let (tx, rx) = std::sync::mpsc::channel();

    unsafe {
        let cls = match objc::runtime::Class::get("AVCaptureDevice") {
            Some(c) => c,
            None => {
                log::error!("AVCaptureDevice class not found");
                return false;
            }
        };

        let media_type: *mut objc::runtime::Object =
            msg_send![class!(NSString), stringWithUTF8String: c"soun".as_ptr()];

        let block = block::ConcreteBlock::new(move |granted: bool| {
            let _ = tx.send(granted);
        });
        let block = block.copy();

        let _: () = msg_send![
            cls,
            requestAccessForMediaType: media_type
            completionHandler: &*block
        ];
    }

    // Wait for the user to respond to the dialog
    rx.recv().unwrap_or(false)
}

/// Open System Preferences to the Microphone privacy pane
pub fn open_microphone_settings() -> Result<()> {
    std::process::Command::new("open")
        .arg("x-apple.systempreferences:com.apple.preference.security?Privacy_Microphone")
        .spawn()?;
    Ok(())
}

/// Check if the app has Accessibility permission (needed by `enigo` to post
/// Cmd+V keystrokes system-wide).
///
/// IMPORTANT: Accessibility is the **write** side of HID — it lets the app
/// *send* events to other apps. **Listening** for events (what CGEventTap
/// does for Fn detection) is gated by **Input Monitoring** instead — see
/// `is_input_monitoring_trusted` below. Historically the two were conflated
/// under Accessibility; on macOS 10.15+ they are separate TCC services, and
/// on modern macOS the keyboard event tap requires Input Monitoring.
///
/// The API (`AXIsProcessTrustedWithOptions`) is the authoritative source of
/// truth. An earlier version of this function treated a "tap created
/// successfully" probe as ground truth — that was wrong, because a
/// `Session` + `ListenOnly` + `FlagsChanged` tap *creates* without any TCC
/// grant; it just silently receives no events from other apps. The probe
/// was therefore a false positive: the app reported "accessibility granted"
/// when in fact nothing had been granted, and the user got no prompt and no
/// banner, only a non-functional Fn key.
pub fn is_accessibility_trusted() -> bool {
    extern "C" {
        // Returns Boolean (unsigned char), NOT Rust bool.
        fn AXIsProcessTrustedWithOptions(options: *const std::ffi::c_void) -> u8;
    }

    let key = CFString::new("AXTrustedCheckOptionPrompt");
    let value = CFBoolean::false_value();
    let options = CFDictionary::from_CFType_pairs(&[(key.as_CFType(), value.as_CFType())]);

    let trusted =
        unsafe { AXIsProcessTrustedWithOptions(options.as_concrete_TypeRef() as *const _) != 0 };

    // Log alongside the running binary path so we can diagnose the two common
    // failure modes when the user reports "I toggled it on but the app says
    // off": (1) per-process AX trust caching — the result keeps reading false
    // until the process restarts; (2) identity mismatch — System Settings
    // shows "Magpie.app" but the running dev binary at target/debug/magpie is
    // a different TCC identity, so the toggle gates a different binary.
    log::info!(
        "is_accessibility_trusted: {} (exe={:?})",
        trusted,
        std::env::current_exe().ok()
    );

    trusted
}

/// Open System Settings to the Accessibility privacy pane
pub fn open_accessibility_settings() -> Result<()> {
    std::process::Command::new("open")
        .arg("x-apple.systempreferences:com.apple.preference.security?Privacy_Accessibility")
        .spawn()?;
    Ok(())
}

// ── Input Monitoring (kTCCServiceListenEvent) ──────────────────────
//
// CGEventTap monitoring keyboard events (including modifier keys like Fn
// via `FlagsChanged`) is gated by the "Input Monitoring" TCC service on
// macOS 10.15+, NOT by Accessibility. The IOKit HID APIs below are the
// supported way to check and request it. They prompt the user the first
// time they're called, which also adds the app to the Input Monitoring
// pane in System Settings so the toggle becomes available.

#[link(name = "IOKit", kind = "framework")]
extern "C" {
    // IOHIDRequestType:  0 = kIOHIDRequestTypePostEvent (Accessibility)
    //                    1 = kIOHIDRequestTypeListenEvent (Input Monitoring)
    fn IOHIDCheckAccess(request_type: u32) -> u32;
    // Returns true if access is granted; triggers the TCC prompt if the
    // status is currently Unknown (first run after install).
    fn IOHIDRequestAccess(request_type: u32) -> bool;
}

const IOHID_REQUEST_TYPE_LISTEN_EVENT: u32 = 1;
// IOHIDAccessType values
const IOHID_ACCESS_TYPE_GRANTED: u32 = 0;
// const IOHID_ACCESS_TYPE_DENIED: u32 = 1;
// const IOHID_ACCESS_TYPE_UNKNOWN: u32 = 2;

/// Check if the app has Input Monitoring permission (needed for CGEventTap
/// to receive system-wide keyboard events, including Fn via FlagsChanged).
pub fn is_input_monitoring_trusted() -> bool {
    unsafe { IOHIDCheckAccess(IOHID_REQUEST_TYPE_LISTEN_EVENT) == IOHID_ACCESS_TYPE_GRANTED }
}

/// Request Input Monitoring access. On first call (status Unknown), macOS
/// shows the permission prompt AND adds the app to System Settings →
/// Privacy & Security → Input Monitoring so the toggle becomes available.
/// On subsequent calls with a denied/granted status, it's a no-op returning
/// the current state.
///
/// Returns `true` if access is currently granted.
pub fn request_input_monitoring_access() -> bool {
    unsafe { IOHIDRequestAccess(IOHID_REQUEST_TYPE_LISTEN_EVENT) }
}

/// Open System Settings to the Input Monitoring privacy pane.
pub fn open_input_monitoring_settings() -> Result<()> {
    std::process::Command::new("open")
        .arg("x-apple.systempreferences:com.apple.preference.security?Privacy_ListenEvent")
        .spawn()?;
    Ok(())
}
