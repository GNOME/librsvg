use std::ops;
use std::{f64, i32};

use glib::object::ObjectClass;
use glib::subclass;
use glib::subclass::object::ObjectClassSubclassExt;
use glib::subclass::prelude::*;
use glib::translate::*;
use glib::{ParamFlags, ParamSpec, ToValue};

use glib_sys;
use gobject_sys;

use handle::Handle;

extern "C" {
    fn rsvg_handle_flags_get_type() -> glib_sys::GType;
}

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
            from_glib(unsafe { rsvg_handle_flags_get_type() }),
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
                self.set_load_flags(value.get().expect("flags value has incorrect type"));
            }

            subclass::Property("dpi-x", ..) => {
                self.set_dpi_x(value.get().expect("dpi-x value has incorrect type"));
            }

            subclass::Property("dpi-y", ..) => {
                self.set_dpi_y(value.get().expect("dpi-y value has incorrect type"));
            }

            subclass::Property("base-uri", ..) => {
                self.set_base_url(value.get().expect("base-uri should be a non-NULL string"));
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

            subclass::Property("title", ..) => Ok("".to_value()), // deprecated
            subclass::Property("desc", ..) => Ok("".to_value()),  // deprecated
            subclass::Property("metadata", ..) => Ok("".to_value()), // deprecated

            _ => unreachable!("invalid property id={} for RsvgHandle", id),
        }
    }
}
