//! macOS running-apps enumeration via NSWorkspace.
//!
//! Returns the list of currently running applications that are addressable
//! via a bundle identifier and have some form of UI (Regular or Accessory
//! activation policy). Prohibited (background-only) apps are filtered out
//! because they aren't dictation targets.
//!
//! Used by the Settings → App Profiles picker so the user can pick a target
//! app from a list instead of relying on a frontmost-detection that always
//! reports Magpie itself (since the Settings window holds focus at click
//! time).
//!
//! Does NOT require any TCC entitlement — NSWorkspace queries are unprivileged.

// `cocoa` is in maintenance; objc2 is the suggested replacement. The rest of
// the codebase still uses `cocoa`, so we match that for consistency until a
// repo-wide migration happens.
#![allow(deprecated)]

use base64::{engine::general_purpose::STANDARD as BASE64, Engine as _};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RunningApp {
    pub bundle_id: String,
    pub name: String,
    /// PNG data URL (`data:image/png;base64,...`) — `None` if the icon could
    /// not be extracted. The frontend falls back to a generic placeholder.
    pub icon_data_url: Option<String>,
}

#[cfg(target_os = "macos")]
pub fn list() -> Vec<RunningApp> {
    use cocoa::base::{id, nil};
    use cocoa::foundation::{NSArray, NSAutoreleasePool, NSUInteger};
    use objc::{class, msg_send, sel, sel_impl};

    unsafe {
        let pool: id = NSAutoreleasePool::new(nil);

        let workspace: id = msg_send![class!(NSWorkspace), sharedWorkspace];
        if workspace == nil {
            let _: () = msg_send![pool, drain];
            return Vec::new();
        }

        let apps: id = msg_send![workspace, runningApplications];
        if apps == nil {
            let _: () = msg_send![pool, drain];
            return Vec::new();
        }

        let count: NSUInteger = apps.count();
        let mut out: Vec<RunningApp> = Vec::with_capacity(count as usize);

        for i in 0..count {
            let app: id = apps.objectAtIndex(i);
            if app == nil {
                continue;
            }

            // NSApplicationActivationPolicy:
            //   Regular = 0, Accessory = 1, Prohibited = 2
            // We keep Regular + Accessory (anything with UI a user can dictate into)
            // and drop Prohibited (pure background helpers, launch agents, etc.).
            let policy: i64 = msg_send![app, activationPolicy];
            if policy != 0 && policy != 1 {
                continue;
            }

            let bundle_id_ns: id = msg_send![app, bundleIdentifier];
            let Some(bundle_id) = nsstring_to_string(bundle_id_ns) else {
                continue;
            };

            let name_ns: id = msg_send![app, localizedName];
            let name = nsstring_to_string(name_ns).unwrap_or_else(|| bundle_id.clone());

            let icon_data_url = extract_icon_png_base64(app);

            out.push(RunningApp {
                bundle_id,
                name,
                icon_data_url,
            });
        }

        let _: () = msg_send![pool, drain];
        out
    }
}

#[cfg(target_os = "macos")]
unsafe fn extract_icon_png_base64(running_app: cocoa::base::id) -> Option<String> {
    use cocoa::base::{id, nil};
    use cocoa::foundation::{NSPoint, NSRect, NSSize};
    use objc::{class, msg_send, sel, sel_impl};

    let icon: id = msg_send![running_app, icon];
    if icon == nil {
        return None;
    }

    // Draw the icon into a 32×32 NSImage so the PNG payload stays small
    // (~2-4 KB per icon vs. ~200 KB if we encode the native 1024px source).
    let target_size = NSSize::new(32.0, 32.0);

    let target: id = msg_send![class!(NSImage), alloc];
    let target: id = msg_send![target, initWithSize: target_size];
    if target == nil {
        return None;
    }

    let _: () = msg_send![target, lockFocus];
    let target_rect = NSRect::new(NSPoint::new(0.0, 0.0), target_size);
    let zero_rect = NSRect::new(NSPoint::new(0.0, 0.0), NSSize::new(0.0, 0.0));
    // NSCompositingOperationSourceOver = 2.
    let _: () = msg_send![
        icon,
        drawInRect: target_rect
        fromRect: zero_rect
        operation: 2u64
        fraction: 1.0_f64
    ];
    let _: () = msg_send![target, unlockFocus];

    let tiff_data: id = msg_send![target, TIFFRepresentation];
    if tiff_data == nil {
        return None;
    }

    let bitmap: id = msg_send![class!(NSBitmapImageRep), imageRepWithData: tiff_data];
    if bitmap == nil {
        return None;
    }

    // NSBitmapImageFileTypePNG = 4. `properties` is an NSDictionary; nil means defaults.
    let png_data: id = msg_send![bitmap, representationUsingType: 4u64 properties: nil];
    if png_data == nil {
        return None;
    }

    let length: usize = msg_send![png_data, length];
    if length == 0 {
        return None;
    }
    let bytes: *const u8 = msg_send![png_data, bytes];
    if bytes.is_null() {
        return None;
    }

    let slice = std::slice::from_raw_parts(bytes, length);
    let encoded = BASE64.encode(slice);
    Some(format!("data:image/png;base64,{}", encoded))
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
pub fn list() -> Vec<RunningApp> {
    Vec::new()
}
