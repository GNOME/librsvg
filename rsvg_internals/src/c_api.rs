use std::sync::Once;

use glib_sys;
use gobject_sys::{self, GEnumValue, GFlagsValue};

use error::RSVG_ERROR_FAILED;

#[no_mangle]
pub unsafe extern "C" fn rsvg_rust_error_get_type() -> glib_sys::GType {
    static ONCE: Once = Once::new();
    static mut ETYPE: glib_sys::GType = gobject_sys::G_TYPE_INVALID;

    // We have to store the GEnumValue in a static variable but
    // that requires it to be Sync. It is not Sync by default
    // because it contains pointers, so we have define a custom
    // wrapper type here on which we can implement Sync.
    #[repr(transparent)]
    struct GEnumValueWrapper(GEnumValue);
    unsafe impl Sync for GEnumValueWrapper {}

    static VALUES: [GEnumValueWrapper; 2] = [
        GEnumValueWrapper(GEnumValue {
            value: RSVG_ERROR_FAILED,
            value_name: b"RSVG_ERROR_FAILED\0" as *const u8 as *const _,
            value_nick: b"failed\0" as *const u8 as *const _,
        }),
        GEnumValueWrapper(GEnumValue {
            value: 0,
            value_name: 0 as *const _,
            value_nick: 0 as *const _,
        }),
    ];

    ONCE.call_once(|| {
        ETYPE = gobject_sys::g_enum_register_static(
            b"RsvgError\0" as *const u8 as *const _,
            &VALUES as *const GEnumValueWrapper as *const GEnumValue,
        );
    });

    ETYPE
}

#[no_mangle]
pub unsafe extern "C" fn rsvg_rust_handle_flags_get_type() -> glib_sys::GType {
    static ONCE: Once = Once::new();
    static mut FTYPE: glib_sys::GType = gobject_sys::G_TYPE_INVALID;

    // We have to store the GFlagsValue in a static variable but
    // that requires it to be Sync. It is not Sync by default
    // because it contains pointers, so we have define a custom
    // wrapper type here on which we can implement Sync.
    #[repr(transparent)]
    struct GFlagsValueWrapper(GFlagsValue);
    unsafe impl Sync for GFlagsValueWrapper {}

    static VALUES: [GFlagsValueWrapper; 4] = [
        GFlagsValueWrapper(GFlagsValue {
            value: 0, // handle_flags::HandleFlags::NONE.bits(),
            value_name: b"RSVG_HANDLE_FLAGS_NONE\0" as *const u8 as *const _,
            value_nick: b"flags-none\0" as *const u8 as *const _,
        }),
        GFlagsValueWrapper(GFlagsValue {
            value: 1 << 0, // HandleFlags::UNLIMITED.to_glib(),
            value_name: b"RSVG_HANDLE_FLAG_UNLIMITED\0" as *const u8 as *const _,
            value_nick: b"flag-unlimited\0" as *const u8 as *const _,
        }),
        GFlagsValueWrapper(GFlagsValue {
            value: 1 << 1, // HandleFlags::KEEP_IMAGE_DATA.to_glib(),
            value_name: b"RSVG_HANDLE_FLAG_KEEP_IMAGE_DATA\0" as *const u8 as *const _,
            value_nick: b"flag-keep-image-data\0" as *const u8 as *const _,
        }),
        GFlagsValueWrapper(GFlagsValue {
            value: 0,
            value_name: 0 as *const _,
            value_nick: 0 as *const _,
        }),
    ];

    ONCE.call_once(|| {
        FTYPE = gobject_sys::g_flags_register_static(
            b"RsvgHandleFlags\0" as *const u8 as *const _,
            &VALUES as *const GFlagsValueWrapper as *const GFlagsValue,
        );
    });

    FTYPE
}
