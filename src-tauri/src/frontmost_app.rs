//! macOS frontmost-app detection via NSWorkspace.
//!
//! Returns `(bundleIdentifier, localizedName)` of the currently frontmost
//! application. `None` if no app is frontmost (e.g. our own Settings window
//! briefly takes focus, or all apps are minimized).
//!
//! Does NOT require any TCC entitlement — NSWorkspace queries are unprivileged.

// `cocoa` is in maintenance; objc2 is the suggested replacement. The rest of
// the codebase still uses `cocoa`, so we match that for consistency until a
// repo-wide migration happens.
#![allow(deprecated)]

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FrontmostApp {
    pub bundle_id: String,
    pub name: String,
}

#[cfg(target_os = "macos")]
pub fn detect() -> Option<FrontmostApp> {
    use cocoa::base::{id, nil};
    use cocoa::foundation::NSAutoreleasePool;
    use objc::{class, msg_send, sel, sel_impl};

    unsafe {
        let pool: id = NSAutoreleasePool::new(nil);

        // [NSWorkspace sharedWorkspace]
        let workspace: id = msg_send![class!(NSWorkspace), sharedWorkspace];
        if workspace == nil {
            let _: () = msg_send![pool, drain];
            return None;
        }

        // [workspace frontmostApplication]
        let app: id = msg_send![workspace, frontmostApplication];
        if app == nil {
            let _: () = msg_send![pool, drain];
            return None;
        }

        let bundle_id_ns: id = msg_send![app, bundleIdentifier];
        let name_ns: id = msg_send![app, localizedName];

        let bundle_id = nsstring_to_string(bundle_id_ns);
        let name = nsstring_to_string(name_ns);

        let _: () = msg_send![pool, drain];

        match (bundle_id, name) {
            (Some(b), Some(n)) => Some(FrontmostApp {
                bundle_id: b,
                name: n,
            }),
            // Some apps (rare) have no bundle ID — skip them; we have no useful key.
            _ => None,
        }
    }
}

#[cfg(target_os = "macos")]
unsafe fn nsstring_to_string(ns_string: cocoa::base::id) -> Option<String> {
    use cocoa::base::nil;
    use cocoa::foundation::NSString;
    if ns_string == nil {
        return None;
    }
    let bytes: *const std::os::raw::c_char = ns_string.UTF8String();
    if bytes.is_null() {
        return None;
    }
    let cstr = std::ffi::CStr::from_ptr(bytes);
    cstr.to_str().ok().map(|s| s.to_string())
}

#[cfg(not(target_os = "macos"))]
pub fn detect() -> Option<FrontmostApp> {
    None
}
