//! Read-only Launch Services lookup for the macOS 11 desktop association fallback.
#![allow(unsafe_code)]

use std::ffi::c_void;

use objc2::rc::Retained;
use objc2_foundation::{NSString, NSURL};

#[link(name = "CoreServices", kind = "framework")]
unsafe extern "C" {
    // Apple declares a borrowed CFStringRef, an UInt32 role mask, an optional
    // CFErrorRef out pointer, and an owned, nullable CFURLRef return value.
    // https://developer.apple.com/documentation/coreservices/1447734-lscopydefaultapplicationurlforco?language=objc
    fn LSCopyDefaultApplicationURLForContentType(
        content_type: *const c_void,
        roles: u32,
        error: *mut *mut c_void,
    ) -> *const c_void;
}

/// Queries the default application for a content type on macOS 11, before AppKit exposed it.
pub fn macos_application_for_content_type(identifier: &NSString) -> Option<Retained<NSURL>> {
    const ALL_ROLES: u32 = u32::MAX;
    // SAFETY: The live NSString is toll-free bridged to CFString and is borrowed
    // for the call. Launch Services accepts a null error pointer; no error is
    // transferred. The returned CFURL is either null or a +1 reference toll-free
    // bridged to NSURL. Retained adopts that reference and releases it on drop.
    unsafe {
        let url = LSCopyDefaultApplicationURLForContentType(
            std::ptr::from_ref(identifier).cast(),
            ALL_ROLES,
            std::ptr::null_mut(),
        );
        Retained::from_raw(url.cast_mut().cast::<NSURL>())
    }
}
