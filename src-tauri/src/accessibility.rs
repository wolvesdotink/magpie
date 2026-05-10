//! Safe wrappers around macOS Accessibility API (AXUIElement).
//!
//! Used to read the text content of the focused UI element in other
//! applications, enabling automatic correction detection.

use core_foundation::base::{CFRelease, CFTypeRef, TCFType};
use core_foundation::string::{CFString, CFStringRef};
use std::ffi::c_void;
use std::ptr;

/// Opaque type for AXUIElement references
#[allow(non_camel_case_types)]
type AXUIElementRef = *const c_void;

/// AXError type (Int32)
#[allow(non_camel_case_types)]
type AXError = i32;

const K_AX_ERROR_SUCCESS: AXError = 0;

// Link against ApplicationServices framework
#[link(name = "ApplicationServices", kind = "framework")]
extern "C" {
    fn AXUIElementCreateSystemWide() -> AXUIElementRef;
    fn AXUIElementCopyAttributeValue(
        element: AXUIElementRef,
        attribute: CFStringRef,
        value: *mut CFTypeRef,
    ) -> AXError;
}

/// Get the text value of the currently focused UI element.
///
/// Returns `None` if:
/// - No element is focused
/// - The focused element has no text value
/// - Accessibility permissions are not granted
/// - The target app doesn't support the AX API properly
pub fn get_focused_element_value() -> Option<String> {
    unsafe {
        // Get the system-wide accessibility element
        let system_wide = AXUIElementCreateSystemWide();
        if system_wide.is_null() {
            log::debug!("AX: Failed to create system-wide element");
            return None;
        }

        // Get the focused UI element
        let focused_attr = CFString::new("AXFocusedUIElement");
        let mut focused_value: CFTypeRef = ptr::null();
        let err = AXUIElementCopyAttributeValue(
            system_wide,
            focused_attr.as_concrete_TypeRef(),
            &mut focused_value,
        );
        CFRelease(system_wide as CFTypeRef);

        if err != K_AX_ERROR_SUCCESS || focused_value.is_null() {
            log::debug!("AX: No focused element (error: {})", err);
            return None;
        }

        let focused_element = focused_value as AXUIElementRef;

        // Get the text value of the focused element
        let value_attr = CFString::new("AXValue");
        let mut text_value: CFTypeRef = ptr::null();
        let err = AXUIElementCopyAttributeValue(
            focused_element,
            value_attr.as_concrete_TypeRef(),
            &mut text_value,
        );
        CFRelease(focused_value);

        if err != K_AX_ERROR_SUCCESS || text_value.is_null() {
            log::debug!("AX: No text value on focused element (error: {})", err);
            return None;
        }

        // Convert CFStringRef to Rust String
        let cf_string = CFString::wrap_under_create_rule(text_value as CFStringRef);
        let result = cf_string.to_string();

        Some(result)
    }
}

#[cfg(test)]
mod tests {
    // Note: AX API tests require a running macOS GUI session with
    // accessibility permissions. These are integration tests that
    // must be run manually.
    //
    // #[test]
    // fn test_get_focused_value() {
    //     let value = super::get_focused_element_value();
    //     println!("Focused element value: {:?}", value);
    // }
}
