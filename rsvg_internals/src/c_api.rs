use std::ops;
use std::sync::Once;
use std::{f64, i32};

use libc;

use glib::object::ObjectClass;
use glib::subclass;
use glib::subclass::object::ObjectClassSubclassExt;
use glib::subclass::prelude::*;
use glib::translate::*;
use glib::value::{FromValue, FromValueOptional, SetValue};
use glib::{ParamFlags, ParamSpec, StaticType, ToValue, Type, Value};

use glib_sys;
use gobject_sys::{self, GEnumValue, GFlagsValue};

use error::RSVG_ERROR_FAILED;
use handle::Handle;

mod handle_flags {
    // The following is entirely stolen from the auto-generated code
    // for GBindingFlags, from gtk-rs/glib/src/gobject/auto/flags.rs

    use super::*;

    // Keep these in sync with rsvg.h:RsvgHandleFlags
    #[cfg_attr(rustfmt, rustfmt_skip)]
    bitflags! {
        pub struct HandleFlags: u32 {
            const NONE            = 0;
            const UNLIMITED       = 1 << 0;
            const KEEP_IMAGE_DATA = 1 << 1;
        }
    }

    pub type RsvgHandleFlags = libc::c_uint;

    impl ToGlib for HandleFlags {
        type GlibType = RsvgHandleFlags;

        fn to_glib(&self) -> RsvgHandleFlags {
            self.bits()
        }
    }

    impl FromGlib<RsvgHandleFlags> for HandleFlags {
        fn from_glib(value: RsvgHandleFlags) -> HandleFlags {
            HandleFlags::from_bits_truncate(value)
        }
    }

    impl StaticType for HandleFlags {
        fn static_type() -> Type {
            unsafe { from_glib(rsvg_rust_handle_flags_get_type()) }
        }
    }

    impl<'a> FromValueOptional<'a> for HandleFlags {
        unsafe fn from_value_optional(value: &Value) -> Option<Self> {
            Some(FromValue::from_value(value))
        }
    }

    impl<'a> FromValue<'a> for HandleFlags {
        unsafe fn from_value(value: &Value) -> Self {
            from_glib(gobject_sys::g_value_get_flags(value.to_glib_none().0))
        }
    }

    impl SetValue for HandleFlags {
        unsafe fn set_value(value: &mut Value, this: &Self) {
            gobject_sys::g_value_set_flags(value.to_glib_none_mut().0, this.to_glib())
        }
    }
}

pub use self::handle_flags::*;

// Keep this in sync with rsvg.h:RsvgHandleClass
#[repr(C)]
pub struct RsvgHandleClass {
    parent: gobject_sys::GObjectClass,

    _abi_padding: [glib_sys::gpointer; 15],
}

// Keep this in sync with rsvg.h:RsvgHandle
#[repr(C)]
pub struct RsvgHandle {
    parent: gobject_sys::GObject,

    _abi_padding: [glib_sys::gpointer; 16],
}

unsafe impl ClassStruct for RsvgHandleClass {
    type Type = Handle;
}

unsafe impl InstanceStruct for RsvgHandle {
    type Type = Handle;
}

impl ops::Deref for RsvgHandleClass {
    type Target = ObjectClass;

    fn deref(&self) -> &Self::Target {
        unsafe { &*(self as *const _ as *const Self::Target) }
    }
}

impl ops::DerefMut for RsvgHandleClass {
    fn deref_mut(&mut self) -> &mut Self::Target {
        unsafe { &mut *(self as *mut _ as *mut Self::Target) }
    }
}

static PROPERTIES: [subclass::Property; 11] = [
    subclass::Property("flags", |name| {
        ParamSpec::flags(
            name,
            "Flags",
            "Loading flags",
            HandleFlags::static_type(),
            0,
            ParamFlags::READWRITE | ParamFlags::CONSTRUCT_ONLY,
        )
    }),
    subclass::Property("dpi-x", |name| {
        ParamSpec::double(
            name,
            "Horizontal DPI",
            "Horizontal resolution in dots per inch",
            0.0,
            f64::MAX,
            0.0,
            ParamFlags::READWRITE | ParamFlags::CONSTRUCT,
        )
    }),
    subclass::Property("dpi-y", |name| {
        ParamSpec::double(
            name,
            "Vertical DPI",
            "Vertical resolution in dots per inch",
            0.0,
            f64::MAX,
            0.0,
            ParamFlags::READWRITE | ParamFlags::CONSTRUCT,
        )
    }),
    subclass::Property("base-uri", |name| {
        ParamSpec::string(
            name,
            "Base URI",
            "Base URI for resolving relative references",
            None,
            ParamFlags::READWRITE | ParamFlags::CONSTRUCT,
        )
    }),
    subclass::Property("width", |name| {
        ParamSpec::int(
            name,
            "Image width",
            "Image width",
            0,
            i32::MAX,
            0,
            ParamFlags::READABLE,
        )
    }),
    subclass::Property("height", |name| {
        ParamSpec::int(
            name,
            "Image height",
            "Image height",
            0,
            i32::MAX,
            0,
            ParamFlags::READABLE,
        )
    }),
    subclass::Property("em", |name| {
        ParamSpec::double(name, "em", "em", 0.0, f64::MAX, 0.0, ParamFlags::READABLE)
    }),
    subclass::Property("ex", |name| {
        ParamSpec::double(name, "ex", "ex", 0.0, f64::MAX, 0.0, ParamFlags::READABLE)
    }),
    subclass::Property("title", |name| {
        ParamSpec::string(name, "deprecated", "deprecated", None, ParamFlags::READABLE)
    }),
    subclass::Property("desc", |name| {
        ParamSpec::string(name, "deprecated", "deprecated", None, ParamFlags::READABLE)
    }),
    subclass::Property("metadata", |name| {
        ParamSpec::string(name, "deprecated", "deprecated", None, ParamFlags::READABLE)
    }),
];

impl ObjectSubclass for Handle {
    const NAME: &'static str = "RsvgHandle";

    type ParentType = glib::Object;

    // We don't use subclass:simple::InstanceStruct and ClassStruct
    // because we need to maintain the respective _abi_padding of each
    // of RsvgHandleClass and RsvgHandle.

    type Instance = RsvgHandle;
    type Class = RsvgHandleClass;

    glib_object_subclass!();

    fn class_init(klass: &mut RsvgHandleClass) {
        klass.install_properties(&PROPERTIES);
    }

    fn new() -> Self {
        Handle::new()
    }
}

impl ObjectImpl for Handle {
    glib_object_impl!();

    fn set_property(&self, _obj: &glib::Object, id: usize, value: &glib::Value) {
        let prop = &PROPERTIES[id];

        match *prop {
            subclass::Property("flags", ..) => {
                let v: HandleFlags = value.get().expect("flags value has incorrect type");

                self.set_load_flags(v);
            }

            subclass::Property("dpi-x", ..) => {
                self.set_dpi_x(value.get().expect("dpi-x value has incorrect type"));
            }

            subclass::Property("dpi-y", ..) => {
                self.set_dpi_y(value.get().expect("dpi-y value has incorrect type"));
            }

            subclass::Property("base-uri", ..) => {
                let v: Option<String> = value.get();

                // rsvg_handle_set_base_uri() expects non-NULL URI strings,
                // but the "base-uri" property can be set to NULL due to a missing
                // construct-time property.

                if let Some(s) = v {
                    self.set_base_url(&s);
                }
            }

            _ => unreachable!("invalid property id {}", id),
        }
    }

    fn get_property(&self, _obj: &glib::Object, id: usize) -> Result<glib::Value, ()> {
        let prop = &PROPERTIES[id];

        match *prop {
            subclass::Property("flags", ..) => Ok(self.load_flags.get().to_flags().to_value()),
            subclass::Property("dpi-x", ..) => Ok(self.dpi.get().x().to_value()),
            subclass::Property("dpi-y", ..) => Ok(self.dpi.get().y().to_value()),

            subclass::Property("base-uri", ..) => Ok(self
                .base_url
                .borrow()
                .as_ref()
                .map(|url| url.as_str())
                .to_value()),

            subclass::Property("width", ..) => Ok(self.get_dimensions_no_error().width.to_value()),
            subclass::Property("height", ..) => {
                Ok(self.get_dimensions_no_error().height.to_value())
            }

            subclass::Property("em", ..) => Ok(self.get_dimensions_no_error().em.to_value()),
            subclass::Property("ex", ..) => Ok(self.get_dimensions_no_error().ex.to_value()),

            // the following three are deprecated
            subclass::Property("title", ..) => Ok((None as Option<String>).to_value()),
            subclass::Property("desc", ..) => Ok((None as Option<String>).to_value()),
            subclass::Property("metadata", ..) => Ok((None as Option<String>).to_value()),

            _ => unreachable!("invalid property id={} for RsvgHandle", id),
        }
    }
}

pub fn get_rust_handle<'a>(handle: *const RsvgHandle) -> &'a Handle {
    let handle = unsafe { &*handle };
    handle.get_impl()
}

#[no_mangle]
pub unsafe extern "C" fn rsvg_handle_rust_get_type() -> glib_sys::GType {
    Handle::get_type().to_glib()
}

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
