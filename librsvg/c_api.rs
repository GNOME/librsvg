use std::cell::{Ref, RefCell, RefMut};
use std::ffi::{CStr, CString};
use std::ops;
use std::path::PathBuf;
use std::ptr;
use std::slice;
use std::sync::Once;
use std::{f64, i32};

use cairo::{self, ImageSurface};
use cairo_sys;
use gdk_pixbuf::Pixbuf;
use gdk_pixbuf_sys;
use glib::error::ErrorDomain;
use libc;
use url::Url;

use bitflags::bitflags;

use gio::prelude::*;
use gio_sys;

use glib::object::ObjectClass;
use glib::subclass;
use glib::subclass::object::ObjectClassSubclassExt;
use glib::subclass::prelude::*;
use glib::translate::*;
use glib::value::{FromValue, FromValueOptional, SetValue};
use glib::{
    glib_object_impl, glib_object_subclass, Bytes, Cast, ParamFlags, ParamSpec, StaticType,
    ToValue, Type, Value,
};

use glib_sys;
use gobject_sys::{self, GEnumValue, GFlagsValue};

use rsvg_internals::{
    rsvg_log, DefsLookupErrorKind, Dpi, Handle, IntrinsicDimensions,
    LoadOptions, LoadingError, RenderingError, RsvgDimensionData, RsvgLength, RsvgPositionData,
    RsvgSizeFunc, SharedImageSurface, SizeCallback, SurfaceType, ViewBox,
};

use crate::pixbuf_utils::{empty_pixbuf, pixbuf_from_surface};

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

#[derive(Default, Copy, Clone)]
struct LoadFlags {
    pub unlimited_size: bool,
    pub keep_image_data: bool,
}

pub use self::handle_flags::*;

impl From<HandleFlags> for LoadFlags {
    fn from(hflags: HandleFlags) -> LoadFlags {
        LoadFlags {
            unlimited_size: hflags.contains(HandleFlags::UNLIMITED),
            keep_image_data: hflags.contains(HandleFlags::KEEP_IMAGE_DATA),
        }
    }
}

impl From<LoadFlags> for HandleFlags {
    fn from(lflags: LoadFlags) -> HandleFlags {
        let mut hflags = HandleFlags::empty();

        if lflags.unlimited_size {
            hflags.insert(HandleFlags::UNLIMITED);
        }

        if lflags.keep_image_data {
            hflags.insert(HandleFlags::KEEP_IMAGE_DATA);
        }

        hflags
    }
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

enum LoadState {
    // Just created the CHandle
    Start,

    // Being loaded using the legacy write()/close() API
    Loading { buffer: Vec<u8> },

    ClosedOk { handle: Handle },

    ClosedError,
}

/// Holds the base URL for loading a handle, and the C-accessible version of it
///
/// There is a public API to query the base URL, and we need to
/// produce a CString with it.  However, that API returns a
/// *const char, so we need to maintain a long-lived CString along with the
/// internal Url.
#[derive(Default)]
struct BaseUrl {
    inner: Option<BaseUrlInner>,
}

struct BaseUrlInner {
    url: Url,
    cstring: CString,
}

impl BaseUrl {
    fn set(&mut self, url: Url) {
        let cstring = CString::new(url.as_str()).unwrap();

        self.inner = Some(BaseUrlInner { url, cstring });
    }

    fn get(&self) -> Option<&Url> {
        self.inner.as_ref().map(|b| &b.url)
    }

    fn get_ptr(&self) -> *const libc::c_char {
        self.inner
            .as_ref()
            .map(|b| b.cstring.as_ptr())
            .unwrap_or_else(|| ptr::null())
    }
}

#[derive(Default, Clone, Copy, Debug, PartialEq)]
#[repr(C)]
pub struct RsvgRectangle {
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
}

impl From<cairo::Rectangle> for RsvgRectangle {
    fn from(r: cairo::Rectangle) -> RsvgRectangle {
        RsvgRectangle {
            x: r.x,
            y: r.y,
            width: r.width,
            height: r.height,
        }
    }
}

impl From<RsvgRectangle> for cairo::Rectangle {
    fn from(r: RsvgRectangle) -> cairo::Rectangle {
        cairo::Rectangle {
            x: r.x,
            y: r.y,
            width: r.width,
            height: r.height,
        }
    }
}

impl From<ViewBox> for RsvgRectangle {
    fn from(vb: ViewBox) -> RsvgRectangle {
        RsvgRectangle {
            x: vb.x,
            y: vb.y,
            width: vb.width,
            height: vb.height,
        }
    }
}

/// Contains all the interior mutability for a RsvgHandle to be called
/// from the C API.
pub struct CHandle {
    inner: RefCell<CHandleInner>,
    load_state: RefCell<LoadState>,
}

struct CHandleInner {
    dpi: Dpi,
    load_flags: LoadFlags,
    base_url: BaseUrl,
    size_callback: SizeCallback,
    is_testing: bool,
}

unsafe impl ClassStruct for RsvgHandleClass {
    type Type = CHandle;
}

unsafe impl InstanceStruct for RsvgHandle {
    type Type = CHandle;
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

impl ObjectSubclass for CHandle {
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
        CHandle {
            inner: RefCell::new(CHandleInner {
                dpi: Dpi::default(),
                load_flags: LoadFlags::default(),
                base_url: BaseUrl::default(),
                size_callback: SizeCallback::default(),
                is_testing: false,
            }),
            load_state: RefCell::new(LoadState::Start),
        }
    }
}

impl ObjectImpl for CHandle {
    glib_object_impl!();

    fn set_property(&self, _obj: &glib::Object, id: usize, value: &glib::Value) {
        let prop = &PROPERTIES[id];

        match *prop {
            subclass::Property("flags", ..) => {
                let v: HandleFlags = value.get().expect("flags value has incorrect type");
                self.set_flags(v);
            }

            subclass::Property("dpi-x", ..) => {
                let dpi_x: f64 = value.get().expect("dpi-x value has incorrect type");
                self.set_dpi_x(dpi_x);
            }

            subclass::Property("dpi-y", ..) => {
                let dpi_y: f64 = value.get().expect("dpi-y value has incorrect type");
                self.set_dpi_y(dpi_y);
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

    #[cfg_attr(rustfmt, rustfmt_skip)]
    fn get_property(&self, _obj: &glib::Object, id: usize) -> Result<glib::Value, ()> {
        let prop = &PROPERTIES[id];

        match *prop {
            subclass::Property("flags", ..) => Ok(self.get_flags().to_value()),

            subclass::Property("dpi-x", ..) => Ok(self.get_dpi_x().to_value()),
            subclass::Property("dpi-y", ..) => Ok(self.get_dpi_y().to_value()),

            subclass::Property("base-uri", ..) => Ok(self.get_base_url().to_value()),

            subclass::Property("width", ..) =>
                Ok(self.get_dimensions_or_empty().width.to_value()),

            subclass::Property("height", ..) =>
                Ok(self.get_dimensions_or_empty().height.to_value()),

            subclass::Property("em", ..) =>
                Ok(self.get_dimensions_or_empty().em.to_value()),

            subclass::Property("ex", ..) =>
                Ok(self.get_dimensions_or_empty().ex.to_value()),

            // the following three are deprecated
            subclass::Property("title", ..)    => Ok((None as Option<String>).to_value()),
            subclass::Property("desc", ..)     => Ok((None as Option<String>).to_value()),
            subclass::Property("metadata", ..) => Ok((None as Option<String>).to_value()),

            _ => unreachable!("invalid property id={} for RsvgHandle", id),
        }
    }
}

impl CHandle {
    fn set_base_url(&self, url: &str) {
        let state = self.load_state.borrow();

        match *state {
            LoadState::Start => (),
            _ => {
                rsvg_g_critical(
                    "Please set the base file or URI before loading any data into RsvgHandle",
                );
                return;
            }
        }

        match Url::parse(&url) {
            Ok(u) => {
                rsvg_log!("setting base_uri to \"{}\"", u.as_str());
                let mut inner = self.inner.borrow_mut();
                inner.base_url.set(u);
            }

            Err(e) => {
                rsvg_log!(
                    "not setting base_uri to \"{}\" since it is invalid: {}",
                    url,
                    e
                );
            }
        }
    }

    fn set_base_gfile(&self, file: &gio::File) {
        self.set_base_url(&file.get_uri());
    }

    fn get_base_url(&self) -> Option<String> {
        let inner = self.inner.borrow();
        inner.base_url.get().map(|url| url.as_str().to_string())
    }

    fn get_base_url_as_ptr(&self) -> *const libc::c_char {
        let inner = self.inner.borrow();
        inner.base_url.get_ptr()
    }

    fn set_dpi_x(&self, dpi_x: f64) {
        let mut inner = self.inner.borrow_mut();
        let dpi = inner.dpi;
        inner.dpi = Dpi::new(dpi_x, dpi.y());
    }

    fn set_dpi_y(&self, dpi_y: f64) {
        let mut inner = self.inner.borrow_mut();
        let dpi = inner.dpi;
        inner.dpi = Dpi::new(dpi.x(), dpi_y);
    }

    fn get_dpi_x(&self) -> f64 {
        let inner = self.inner.borrow();
        inner.dpi.x()
    }

    fn get_dpi_y(&self) -> f64 {
        let inner = self.inner.borrow();
        inner.dpi.y()
    }

    fn set_flags(&self, flags: HandleFlags) {
        let mut inner = self.inner.borrow_mut();
        inner.load_flags = LoadFlags::from(flags);
    }

    fn get_flags(&self) -> HandleFlags {
        let inner = self.inner.borrow();
        HandleFlags::from(inner.load_flags)
    }

    fn load_options(&self) -> LoadOptions {
        let inner = self.inner.borrow();

        LoadOptions::new(inner.base_url.get().map(|u| (*u).clone()))
            .with_unlimited_size(inner.load_flags.unlimited_size)
            .keep_image_data(inner.load_flags.keep_image_data)
    }

    fn set_size_callback(
        &self,
        size_func: RsvgSizeFunc,
        user_data: glib_sys::gpointer,
        destroy_notify: glib_sys::GDestroyNotify,
    ) {
        let mut inner = self.inner.borrow_mut();
        inner.size_callback = SizeCallback::new(size_func, user_data, destroy_notify);
    }

    fn write(&self, buf: &[u8]) {
        let mut state = self.load_state.borrow_mut();

        match *state {
            LoadState::Start => {
                *state = LoadState::Loading {
                    buffer: Vec::from(buf),
                }
            }

            LoadState::Loading { ref mut buffer } => {
                buffer.extend_from_slice(buf);
            }

            _ => {
                rsvg_g_critical("Handle must not be closed in order to write to it");
                return;
            }
        }
    }

    fn close(&self) -> Result<(), LoadingError> {
        let mut state = self.load_state.borrow_mut();

        match *state {
            LoadState::Start => {
                *state = LoadState::ClosedError;
                Err(LoadingError::NoDataPassedToParser)
            }

            LoadState::Loading { ref buffer } => {
                let bytes = Bytes::from(&*buffer);
                let stream = gio::MemoryInputStream::new_from_bytes(&bytes);

                self.read_stream(state, &stream.upcast(), None)
            }

            // Closing is idempotent
            LoadState::ClosedOk { .. } => Ok(()),
            LoadState::ClosedError => Ok(()),
        }
    }

    fn read_stream_sync(
        &self,
        stream: &gio::InputStream,
        cancellable: Option<&gio::Cancellable>,
    ) -> Result<(), LoadingError> {
        let state = self.load_state.borrow_mut();

        match *state {
            LoadState::Start => self.read_stream(state, stream, cancellable),
            LoadState::Loading { .. } | LoadState::ClosedOk { .. } | LoadState::ClosedError => {
                rsvg_g_critical(
                    "handle must not be already loaded in order to call \
                     rsvg_handle_read_stream_sync()",
                );
                Err(LoadingError::Unknown)
            }
        }
    }

    fn read_stream(
        &self,
        mut load_state: RefMut<LoadState>,
        stream: &gio::InputStream,
        cancellable: Option<&gio::Cancellable>,
    ) -> Result<(), LoadingError> {
        match Handle::from_stream(&self.load_options(), stream, cancellable) {
            Ok(handle) => {
                *load_state = LoadState::ClosedOk { handle };
                Ok(())
            }

            Err(e) => {
                *load_state = LoadState::ClosedError;
                Err(e)
            }
        }
    }

    fn get_handle_ref(&self) -> Result<Ref<Handle>, RenderingError> {
        let state = self.load_state.borrow();

        match *state {
            LoadState::Start => {
                rsvg_g_critical("Handle has not been loaded");
                Err(RenderingError::HandleIsNotLoaded)
            }

            LoadState::Loading { .. } => {
                rsvg_g_critical("Handle is still loading; call rsvg_handle_close() first");
                Err(RenderingError::HandleIsNotLoaded)
            }

            LoadState::ClosedError => {
                rsvg_g_critical(
                    "Handle could not read or parse the SVG; did you check for errors during the \
                     loading stage?",
                );
                Err(RenderingError::HandleIsNotLoaded)
            }

            LoadState::ClosedOk { .. } => Ok(Ref::map(state, |s| match *s {
                LoadState::ClosedOk { ref handle } => handle,
                _ => unreachable!(),
            })),
        }
    }

    fn has_sub(&self, id: &str) -> Result<bool, RenderingError> {
        let handle = self.get_handle_ref()?;
        handle.has_sub(id).map_err(warn_on_invalid_id)
    }

    fn get_dimensions_or_empty(&self) -> RsvgDimensionData {
        self.get_dimensions()
            .unwrap_or_else(|_| RsvgDimensionData::empty())
    }

    fn get_dimensions(&self) -> Result<RsvgDimensionData, RenderingError> {
        let handle = self.get_handle_ref()?;
        let inner = self.inner.borrow();
        handle.get_dimensions(inner.dpi, &inner.size_callback, inner.is_testing)
    }

    fn get_dimensions_sub(&self, id: Option<&str>) -> Result<RsvgDimensionData, RenderingError> {
        let handle = self.get_handle_ref()?;
        let inner = self.inner.borrow();
        handle
            .get_dimensions_sub(id, inner.dpi, &inner.size_callback, inner.is_testing)
            .map_err(warn_on_invalid_id)
    }

    fn get_position_sub(&self, id: Option<&str>) -> Result<RsvgPositionData, RenderingError> {
        let handle = self.get_handle_ref()?;
        let inner = self.inner.borrow();
        handle
            .get_position_sub(id, inner.dpi, &inner.size_callback, inner.is_testing)
            .map_err(warn_on_invalid_id)
    }

    fn render_cairo_sub(
        &self,
        cr: &cairo::Context,
        id: Option<&str>,
    ) -> Result<(), RenderingError> {
        check_cairo_context(cr)?;

        let handle = self.get_handle_ref()?;
        let inner = self.inner.borrow();
        handle
            .render_cairo_sub(cr, id, inner.dpi, &inner.size_callback, inner.is_testing)
            .map_err(warn_on_invalid_id)
    }

    fn get_pixbuf_sub(&self, id: Option<&str>) -> Result<Pixbuf, RenderingError> {
        let handle = self.get_handle_ref()?;
        let inner = self.inner.borrow();

        let dpi = inner.dpi;
        let is_testing = inner.is_testing;

        let dimensions = handle.get_dimensions(dpi, &inner.size_callback, is_testing)?;

        if dimensions.width == 0 || dimensions.height == 0 {
            return empty_pixbuf();
        }

        let surface =
            ImageSurface::create(cairo::Format::ARgb32, dimensions.width, dimensions.height)?;

        {
            let cr = cairo::Context::new(&surface);
            handle
                .render_cairo_sub(&cr, id, dpi, &inner.size_callback, is_testing)
                .map_err(warn_on_invalid_id)?;
        }

        let surface = SharedImageSurface::new(surface, SurfaceType::SRgb)?;

        pixbuf_from_surface(&surface)
    }

    fn render_document(
        &self,
        cr: &cairo::Context,
        viewport: &cairo::Rectangle,
    ) -> Result<(), RenderingError> {
        check_cairo_context(cr)?;

        let handle = self.get_handle_ref()?;
        let inner = self.inner.borrow();
        handle.render_document(cr, viewport, inner.dpi, inner.is_testing)
    }

    fn get_geometry_for_layer(
        &self,
        id: Option<&str>,
        viewport: &cairo::Rectangle,
    ) -> Result<(RsvgRectangle, RsvgRectangle), RenderingError> {
        let handle = self.get_handle_ref()?;
        let inner = self.inner.borrow();
        handle
            .get_geometry_for_layer(id, viewport, inner.dpi, inner.is_testing)
            .map(|(i, l)| (RsvgRectangle::from(i), RsvgRectangle::from(l)))
            .map_err(warn_on_invalid_id)
    }

    fn render_layer(
        &self,
        cr: &cairo::Context,
        id: Option<&str>,
        viewport: &cairo::Rectangle,
    ) -> Result<(), RenderingError> {
        check_cairo_context(cr)?;

        let handle = self.get_handle_ref()?;
        let inner = self.inner.borrow();
        handle
            .render_layer(cr, id, viewport, inner.dpi, inner.is_testing)
            .map_err(warn_on_invalid_id)
    }

    fn get_geometry_for_element(
        &self,
        id: Option<&str>,
    ) -> Result<(RsvgRectangle, RsvgRectangle), RenderingError> {
        let handle = self.get_handle_ref()?;
        let inner = self.inner.borrow();
        handle
            .get_geometry_for_element(id, inner.dpi, inner.is_testing)
            .map(|(i, l)| (RsvgRectangle::from(i), RsvgRectangle::from(l)))
            .map_err(warn_on_invalid_id)
    }

    fn render_element(
        &self,
        cr: &cairo::Context,
        id: Option<&str>,
        element_viewport: &cairo::Rectangle,
    ) -> Result<(), RenderingError> {
        check_cairo_context(cr)?;

        let handle = self.get_handle_ref()?;
        let inner = self.inner.borrow();
        handle
            .render_element(cr, id, element_viewport, inner.dpi, inner.is_testing)
            .map_err(warn_on_invalid_id)
    }

    fn get_intrinsic_dimensions(&self) -> Result<IntrinsicDimensions, RenderingError> {
        let handle = self.get_handle_ref()?;
        Ok(handle.get_intrinsic_dimensions())
    }

    fn set_testing(&self, is_testing: bool) {
        let mut inner = self.inner.borrow_mut();
        inner.is_testing = is_testing;
    }
}

fn get_rust_handle<'a>(handle: *const RsvgHandle) -> &'a CHandle {
    let handle = unsafe { &*handle };
    handle.get_impl()
}

#[no_mangle]
pub unsafe extern "C" fn rsvg_rust_handle_get_type() -> glib_sys::GType {
    CHandle::get_type().to_glib()
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

#[no_mangle]
pub unsafe extern "C" fn rsvg_rust_handle_set_base_url(
    raw_handle: *const RsvgHandle,
    uri: *const libc::c_char,
) {
    let rhandle = get_rust_handle(raw_handle);

    assert!(!uri.is_null());
    let uri: String = from_glib_none(uri);

    rhandle.set_base_url(&uri);
}

#[no_mangle]
pub unsafe extern "C" fn rsvg_rust_handle_set_base_gfile(
    raw_handle: *const RsvgHandle,
    raw_gfile: *mut gio_sys::GFile,
) {
    let rhandle = get_rust_handle(raw_handle);

    assert!(!raw_gfile.is_null());

    let file: gio::File = from_glib_none(raw_gfile);

    rhandle.set_base_gfile(&file);
}

#[no_mangle]
pub unsafe extern "C" fn rsvg_rust_handle_get_base_url(
    raw_handle: *const RsvgHandle,
) -> *const libc::c_char {
    let rhandle = get_rust_handle(raw_handle);

    rhandle.get_base_url_as_ptr()
}

#[no_mangle]
pub unsafe extern "C" fn rsvg_rust_handle_set_dpi_x(raw_handle: *const RsvgHandle, dpi_x: f64) {
    let rhandle = get_rust_handle(raw_handle);
    rhandle.set_dpi_x(dpi_x);
}

#[no_mangle]
pub unsafe extern "C" fn rsvg_rust_handle_get_dpi_x(raw_handle: *const RsvgHandle) -> f64 {
    let rhandle = get_rust_handle(raw_handle);
    rhandle.get_dpi_x()
}

#[no_mangle]
pub unsafe extern "C" fn rsvg_rust_handle_set_dpi_y(raw_handle: *const RsvgHandle, dpi_y: f64) {
    let rhandle = get_rust_handle(raw_handle);
    rhandle.set_dpi_y(dpi_y);
}

#[no_mangle]
pub unsafe extern "C" fn rsvg_rust_handle_get_dpi_y(raw_handle: *const RsvgHandle) -> f64 {
    let rhandle = get_rust_handle(raw_handle);
    rhandle.get_dpi_y()
}

#[no_mangle]
pub unsafe extern "C" fn rsvg_rust_handle_set_size_callback(
    raw_handle: *const RsvgHandle,
    size_func: RsvgSizeFunc,
    user_data: glib_sys::gpointer,
    destroy_notify: glib_sys::GDestroyNotify,
) {
    let rhandle = get_rust_handle(raw_handle);

    rhandle.set_size_callback(size_func, user_data, destroy_notify);
}

#[no_mangle]
pub unsafe extern "C" fn rsvg_rust_handle_set_testing(
    raw_handle: *const RsvgHandle,
    testing: glib_sys::gboolean,
) {
    let rhandle = get_rust_handle(raw_handle);

    rhandle.set_testing(from_glib(testing));
}

#[no_mangle]
pub unsafe extern "C" fn rsvg_rust_handle_read_stream_sync(
    handle: *const RsvgHandle,
    stream: *mut gio_sys::GInputStream,
    cancellable: *mut gio_sys::GCancellable,
    error: *mut *mut glib_sys::GError,
) -> glib_sys::gboolean {
    let rhandle = get_rust_handle(handle);

    let stream = gio::InputStream::from_glib_none(stream);
    let cancellable: Option<gio::Cancellable> = from_glib_none(cancellable);

    match rhandle.read_stream_sync(&stream, cancellable.as_ref()) {
        Ok(()) => true.to_glib(),

        Err(e) => {
            set_gerror(error, 0, &format!("{}", e));
            false.to_glib()
        }
    }
}

#[no_mangle]
pub unsafe extern "C" fn rsvg_rust_handle_write(
    handle: *const RsvgHandle,
    buf: *const u8,
    count: usize,
) {
    let rhandle = get_rust_handle(handle);
    let buffer = slice::from_raw_parts(buf, count);
    rhandle.write(buffer);
}

#[no_mangle]
pub unsafe extern "C" fn rsvg_rust_handle_close(
    handle: *const RsvgHandle,
    error: *mut *mut glib_sys::GError,
) -> glib_sys::gboolean {
    let rhandle = get_rust_handle(handle);

    match rhandle.close() {
        Ok(()) => true.to_glib(),

        Err(e) => {
            set_gerror(error, 0, &format!("{}", e));
            false.to_glib()
        }
    }
}
#[no_mangle]
pub unsafe extern "C" fn rsvg_rust_handle_has_sub(
    handle: *const RsvgHandle,
    id: *const libc::c_char,
) -> glib_sys::gboolean {
    let rhandle = get_rust_handle(handle);

    if id.is_null() {
        return false.to_glib();
    }

    let id: String = from_glib_none(id);
    rhandle.has_sub(&id).unwrap_or(false).to_glib()
}

#[no_mangle]
pub unsafe extern "C" fn rsvg_rust_handle_render_cairo_sub(
    handle: *const RsvgHandle,
    cr: *mut cairo_sys::cairo_t,
    id: *const libc::c_char,
) -> glib_sys::gboolean {
    let rhandle = get_rust_handle(handle);
    let cr = from_glib_none(cr);
    let id: Option<String> = from_glib_none(id);

    match rhandle.render_cairo_sub(&cr, id.as_ref().map(String::as_str)) {
        Ok(()) => true.to_glib(),

        Err(e) => {
            rsvg_log!("could not render: {}", e);
            false.to_glib()
        }
    }
}

#[no_mangle]
pub unsafe extern "C" fn rsvg_rust_handle_get_pixbuf_sub(
    handle: *const RsvgHandle,
    id: *const libc::c_char,
) -> *mut gdk_pixbuf_sys::GdkPixbuf {
    let rhandle = get_rust_handle(handle);
    let id: Option<String> = from_glib_none(id);

    match rhandle.get_pixbuf_sub(id.as_ref().map(String::as_str)) {
        Ok(pixbuf) => pixbuf.to_glib_full(),
        Err(e) => {
            rsvg_log!("could not render: {}", e);
            ptr::null_mut()
        }
    }
}

#[no_mangle]
pub unsafe extern "C" fn rsvg_rust_handle_get_dimensions(
    handle: *const RsvgHandle,
    dimension_data: *mut RsvgDimensionData,
) {
    let rhandle = get_rust_handle(handle);
    *dimension_data = rhandle
        .get_dimensions()
        .unwrap_or_else(|_| RsvgDimensionData::empty());
}

#[no_mangle]
pub unsafe extern "C" fn rsvg_rust_handle_get_dimensions_sub(
    handle: *const RsvgHandle,
    dimension_data: *mut RsvgDimensionData,
    id: *const libc::c_char,
) -> glib_sys::gboolean {
    let rhandle = get_rust_handle(handle);

    let id: Option<String> = from_glib_none(id);

    match rhandle.get_dimensions_sub(id.as_ref().map(String::as_str)) {
        Ok(dimensions) => {
            *dimension_data = dimensions;
            true.to_glib()
        }

        Err(e) => {
            rsvg_log!("could not get dimensions: {}", e);
            *dimension_data = RsvgDimensionData::empty();
            false.to_glib()
        }
    }
}

#[no_mangle]
pub unsafe extern "C" fn rsvg_rust_handle_get_position_sub(
    handle: *const RsvgHandle,
    position_data: *mut RsvgPositionData,
    id: *const libc::c_char,
) -> glib_sys::gboolean {
    let rhandle = get_rust_handle(handle);

    let id: Option<String> = from_glib_none(id);

    match rhandle.get_position_sub(id.as_ref().map(String::as_str)) {
        Ok(position) => {
            *position_data = position;
            true.to_glib()
        }

        Err(e) => {
            let p = &mut *position_data;

            p.x = 0;
            p.y = 0;

            rsvg_log!("could not get position: {}", e);
            false.to_glib()
        }
    }
}

#[no_mangle]
pub unsafe extern "C" fn rsvg_rust_handle_new_with_flags(flags: u32) -> *const RsvgHandle {
    let obj: *mut gobject_sys::GObject =
        glib::Object::new(CHandle::get_type(), &[("flags", &flags)])
            .unwrap()
            .to_glib_full();

    obj as *mut _
}

#[no_mangle]
pub unsafe extern "C" fn rsvg_rust_handle_new_from_file(
    filename: *const libc::c_char,
    error: *mut *mut glib_sys::GError,
) -> *const RsvgHandle {
    let file = match PathOrUrl::new(filename) {
        Ok(PathOrUrl::Path(path)) => gio::File::new_for_path(path),

        Ok(PathOrUrl::Url(url)) => gio::File::new_for_uri(url.as_str()),

        Err(e) => {
            set_gerror(error, 0, &format!("{}", e));
            return ptr::null_mut();
        }
    };

    rsvg_rust_handle_new_from_gfile_sync(file.to_glib_none().0, 0, ptr::null_mut(), error)
}

#[no_mangle]
pub unsafe extern "C" fn rsvg_rust_handle_new_from_gfile_sync(
    file: *mut gio_sys::GFile,
    flags: u32,
    cancellable: *mut gio_sys::GCancellable,
    error: *mut *mut glib_sys::GError,
) -> *const RsvgHandle {
    let raw_handle = rsvg_rust_handle_new_with_flags(flags);

    let rhandle = get_rust_handle(raw_handle);

    let file = gio::File::from_glib_none(file);
    rhandle.set_base_gfile(&file);

    let cancellable: Option<gio::Cancellable> = from_glib_none(cancellable);

    let res = file
        .read(cancellable.as_ref())
        .map_err(|e| LoadingError::from(e))
        .and_then(|stream| rhandle.read_stream_sync(&stream.upcast(), cancellable.as_ref()));

    match res {
        Ok(()) => raw_handle,

        Err(e) => {
            set_gerror(error, 0, &format!("{}", e));
            gobject_sys::g_object_unref(raw_handle as *mut _);
            ptr::null_mut()
        }
    }
}

#[no_mangle]
pub unsafe extern "C" fn rsvg_rust_handle_new_from_stream_sync(
    input_stream: *mut gio_sys::GInputStream,
    base_file: *mut gio_sys::GFile,
    flags: u32,
    cancellable: *mut gio_sys::GCancellable,
    error: *mut *mut glib_sys::GError,
) -> *const RsvgHandle {
    let raw_handle = rsvg_rust_handle_new_with_flags(flags);

    let rhandle = get_rust_handle(raw_handle);

    let base_file: Option<gio::File> = from_glib_none(base_file);
    if let Some(base_file) = base_file {
        rhandle.set_base_gfile(&base_file);
    }

    let stream: gio::InputStream = from_glib_none(input_stream);
    let cancellable: Option<gio::Cancellable> = from_glib_none(cancellable);

    match rhandle.read_stream_sync(&stream, cancellable.as_ref()) {
        Ok(()) => raw_handle,

        Err(e) => {
            set_gerror(error, 0, &format!("{}", e));
            gobject_sys::g_object_unref(raw_handle as *mut _);
            ptr::null_mut()
        }
    }
}

#[no_mangle]
pub unsafe extern "C" fn rsvg_rust_handle_new_from_data(
    data: *mut u8,
    len: usize,
    error: *mut *mut glib_sys::GError,
) -> *const RsvgHandle {
    // We create the MemoryInputStream without the gtk-rs binding because of this:
    //
    // - The binding doesn't provide _new_from_data().  All of the binding's ways to
    // put data into a MemoryInputStream involve copying the data buffer.
    //
    // - We can't use glib::Bytes from the binding either, for the same reason.
    //
    // - For now, we are using the other C-visible constructor, so we need a raw pointer to the
    //   stream, anyway.

    assert!(len <= std::isize::MAX as usize);
    let len = len as isize;

    let raw_stream = gio_sys::g_memory_input_stream_new_from_data(data, len, None);

    let ret = rsvg_rust_handle_new_from_stream_sync(
        raw_stream as *mut _,
        ptr::null_mut(), // base_file
        0,               // flags
        ptr::null_mut(), // cancellable
        error,
    );

    gobject_sys::g_object_unref(raw_stream as *mut _);
    ret
}

unsafe fn set_out_param<T: Copy>(
    out_has_param: *mut glib_sys::gboolean,
    out_param: *mut T,
    value: &Option<T>,
) {
    let has_value = if let Some(ref v) = *value {
        if !out_param.is_null() {
            *out_param = *v;
        }

        true
    } else {
        false
    };

    if !out_has_param.is_null() {
        *out_has_param = has_value.to_glib();
    }
}

#[no_mangle]
pub unsafe extern "C" fn rsvg_rust_handle_get_intrinsic_dimensions(
    handle: *mut RsvgHandle,
    out_has_width: *mut glib_sys::gboolean,
    out_width: *mut RsvgLength,
    out_has_height: *mut glib_sys::gboolean,
    out_height: *mut RsvgLength,
    out_has_viewbox: *mut glib_sys::gboolean,
    out_viewbox: *mut RsvgRectangle,
) {
    let rhandle = get_rust_handle(handle);

    let d = rhandle
        .get_intrinsic_dimensions()
        .unwrap_or_else(|_| panic!("API called out of order"));

    let w = d.width;
    let h = d.height;
    let r = d.vbox.map(RsvgRectangle::from);

    set_out_param(out_has_width, out_width, &w.map(Into::into));
    set_out_param(out_has_height, out_height, &h.map(Into::into));
    set_out_param(out_has_viewbox, out_viewbox, &r);
}

#[no_mangle]
pub unsafe extern "C" fn rsvg_rust_handle_render_document(
    handle: *mut RsvgHandle,
    cr: *mut cairo_sys::cairo_t,
    viewport: *const RsvgRectangle,
    error: *mut *mut glib_sys::GError,
) -> glib_sys::gboolean {
    let rhandle = get_rust_handle(handle);
    let cr = from_glib_none(cr);

    match rhandle.render_document(&cr, &(*viewport).into()) {
        Ok(()) => true.to_glib(),

        Err(e) => {
            set_gerror(error, 0, &format!("{}", e));
            false.to_glib()
        }
    }
}

#[no_mangle]
pub unsafe extern "C" fn rsvg_rust_handle_get_geometry_for_layer(
    handle: *mut RsvgHandle,
    id: *const libc::c_char,
    viewport: *const RsvgRectangle,
    out_ink_rect: *mut RsvgRectangle,
    out_logical_rect: *mut RsvgRectangle,
    error: *mut *mut glib_sys::GError,
) -> glib_sys::gboolean {
    let rhandle = get_rust_handle(handle);

    let id: Option<String> = from_glib_none(id);

    match rhandle.get_geometry_for_layer(id.as_ref().map(String::as_str), &(*viewport).into()) {
        Ok((ink_rect, logical_rect)) => {
            if !out_ink_rect.is_null() {
                *out_ink_rect = ink_rect;
            }

            if !out_logical_rect.is_null() {
                *out_logical_rect = logical_rect;
            }

            true.to_glib()
        }

        Err(e) => {
            set_gerror(error, 0, &format!("{}", e));
            false.to_glib()
        }
    }
}

#[no_mangle]
pub unsafe extern "C" fn rsvg_rust_handle_render_layer(
    handle: *mut RsvgHandle,
    cr: *mut cairo_sys::cairo_t,
    id: *const libc::c_char,
    viewport: *const RsvgRectangle,
    error: *mut *mut glib_sys::GError,
) -> glib_sys::gboolean {
    let rhandle = get_rust_handle(handle);
    let cr = from_glib_none(cr);
    let id: Option<String> = from_glib_none(id);

    match rhandle.render_layer(&cr, id.as_ref().map(String::as_str), &(*viewport).into()) {
        Ok(()) => true.to_glib(),

        Err(e) => {
            set_gerror(error, 0, &format!("{}", e));
            false.to_glib()
        }
    }
}

#[no_mangle]
pub unsafe extern "C" fn rsvg_rust_handle_get_geometry_for_element(
    handle: *mut RsvgHandle,
    id: *const libc::c_char,
    out_ink_rect: *mut RsvgRectangle,
    out_logical_rect: *mut RsvgRectangle,
    error: *mut *mut glib_sys::GError,
) -> glib_sys::gboolean {
    let rhandle = get_rust_handle(handle);

    let id: Option<String> = from_glib_none(id);

    match rhandle.get_geometry_for_element(id.as_ref().map(String::as_str)) {
        Ok((ink_rect, logical_rect)) => {
            if !out_ink_rect.is_null() {
                *out_ink_rect = ink_rect;
            }

            if !out_logical_rect.is_null() {
                *out_logical_rect = logical_rect;
            }

            true.to_glib()
        }

        Err(e) => {
            set_gerror(error, 0, &format!("{}", e));
            false.to_glib()
        }
    }
}

#[no_mangle]
pub unsafe extern "C" fn rsvg_rust_handle_render_element(
    handle: *mut RsvgHandle,
    cr: *mut cairo_sys::cairo_t,
    id: *const libc::c_char,
    element_viewport: *const RsvgRectangle,
    error: *mut *mut glib_sys::GError,
) -> glib_sys::gboolean {
    let rhandle = get_rust_handle(handle);
    let cr = from_glib_none(cr);
    let id: Option<String> = from_glib_none(id);

    match rhandle.render_element(
        &cr,
        id.as_ref().map(String::as_str),
        &(*element_viewport).into(),
    ) {
        Ok(()) => true.to_glib(),

        Err(e) => {
            set_gerror(error, 0, &format!("{}", e));
            false.to_glib()
        }
    }
}

/// Detects whether a `*const libc::c_char` is a path or a URI
///
/// `rsvg_handle_new_from_file()` takes a `filename` argument, and advertises
/// that it will detect either a file system path, or a proper URI.  It will then use
/// `gio::File::new_for_path()` or `gio::File::new_for_uri()` as appropriate.
///
/// This enum does the magic heuristics to figure this out.
enum PathOrUrl {
    Path(PathBuf),
    Url(Url),
}

impl PathOrUrl {
    unsafe fn new(s: *const libc::c_char) -> Result<PathOrUrl, LoadingError> {
        let cstr = CStr::from_ptr(s);

        Ok(cstr
            .to_str()
            .map_err(|_| ())
            .and_then(|utf8| Url::parse(utf8).map_err(|_| ()))
            .and_then(|url| {
                if url.origin().is_tuple() || url.scheme() == "file" {
                    Ok(PathOrUrl::Url(url))
                } else {
                    Ok(PathOrUrl::Path(url.to_file_path()?))
                }
            })
            .unwrap_or_else(|_| PathOrUrl::Path(PathBuf::from_glib_none(s))))
    }
}

fn check_cairo_context(cr: &cairo::Context) -> Result<(), RenderingError> {
    let status = cr.status();
    if status == cairo::Status::Success {
        Ok(())
    } else {
        let msg = format!(
            "cannot render on a cairo_t with a failure status (status={:?})",
            status,
        );

        rsvg_g_warning(&msg);
        Err(RenderingError::Cairo(status))
    }
}

fn warn_on_invalid_id(e: RenderingError) -> RenderingError {
    if e == RenderingError::InvalidId(DefsLookupErrorKind::CannotLookupExternalReferences) {
        rsvg_g_warning("the public API is not allowed to look up external references");
    }

    e
}

fn rsvg_g_warning(msg: &str) {
    unsafe {
        extern "C" {
            fn rsvg_g_warning_from_c(msg: *const libc::c_char);
        }

        rsvg_g_warning_from_c(msg.to_glib_none().0);
    }
}

fn rsvg_g_critical(msg: &str) {
    unsafe {
        extern "C" {
            fn rsvg_g_critical_from_c(msg: *const libc::c_char);
        }

        rsvg_g_critical_from_c(msg.to_glib_none().0);
    }
}

pub(crate) fn set_gerror(err: *mut *mut glib_sys::GError, code: u32, msg: &str) {
    unsafe {
        // this is RSVG_ERROR_FAILED, the only error code available in RsvgError
        assert!(code == 0);

        glib_sys::g_set_error_literal(
            err,
            rsvg_rust_error_quark(),
            code as libc::c_int,
            msg.to_glib_none().0,
        );
    }
}

/// Used as a generic error to translate to glib::Error
///
/// This type implements `glib::error::ErrorDomain`, so it can be used
/// to obtain the error code while calling `glib::Error::new()`.  Unfortunately
/// the public librsvg API does not have detailed error codes yet, so we use
/// this single value as the only possible error code to return.
#[derive(Copy, Clone)]
pub struct RsvgError;

// Keep in sync with rsvg.h:RsvgError
pub const RSVG_ERROR_FAILED: i32 = 0;

impl ErrorDomain for RsvgError {
    fn domain() -> glib::Quark {
        glib::Quark::from_string("rsvg-error-quark")
    }

    fn code(self) -> i32 {
        RSVG_ERROR_FAILED
    }

    fn from(code: i32) -> Option<Self> {
        match code {
            // We don't have enough information from glib error codes
            _ => Some(RsvgError),
        }
    }
}

#[no_mangle]
pub extern "C" fn rsvg_rust_error_quark() -> glib_sys::GQuark {
    RsvgError::domain().to_glib()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn path_or_url_unix() {
        unsafe {
            match PathOrUrl::new(b"/foo/bar\0" as *const u8 as *const _).unwrap() {
                PathOrUrl::Path(_) => (),
                _ => panic!("unix filename should be a PathOrUrl::Path"),
            }

            match PathOrUrl::new(b"foo/bar\0" as *const u8 as *const _).unwrap() {
                PathOrUrl::Path(_) => (),
                _ => panic!("unix filename should be a PathOrUrl::Path"),
            }
        }
    }

    #[test]
    fn path_or_url_windows() {
        unsafe {
            match PathOrUrl::new(b"c:/foo/bar\0" as *const u8 as *const _).unwrap() {
                PathOrUrl::Path(_) => (),
                _ => panic!("windows filename should be a PathOrUrl::Path"),
            }

            match PathOrUrl::new(b"C:/foo/bar\0" as *const u8 as *const _).unwrap() {
                PathOrUrl::Path(_) => (),
                _ => panic!("windows filename should be a PathOrUrl::Path"),
            }

            match PathOrUrl::new(b"c:\\foo\\bar\0" as *const u8 as *const _).unwrap() {
                PathOrUrl::Path(_) => (),
                _ => panic!("windows filename should be a PathOrUrl::Path"),
            }

            match PathOrUrl::new(b"C:\\foo\\bar\0" as *const u8 as *const _).unwrap() {
                PathOrUrl::Path(_) => (),
                _ => panic!("windows filename should be a PathOrUrl::Path"),
            }
        }
    }

    #[test]
    fn path_or_url_unix_url() {
        unsafe {
            match PathOrUrl::new(b"file:///foo/bar\0" as *const u8 as *const _).unwrap() {
                PathOrUrl::Url(_) => (),
                _ => panic!("file:// unix filename should be a PathOrUrl::Url"),
            }
        }
    }

    #[test]
    fn path_or_url_windows_url() {
        unsafe {
            match PathOrUrl::new(b"file://c:/foo/bar\0" as *const u8 as *const _).unwrap() {
                PathOrUrl::Url(_) => (),
                _ => panic!("file:// windows filename should be a PathOrUrl::Url"),
            }

            match PathOrUrl::new(b"file://C:/foo/bar\0" as *const u8 as *const _).unwrap() {
                PathOrUrl::Url(_) => (),
                _ => panic!("file:// windows filename should be a PathOrUrl::Url"),
            }
        }
    }

    #[test]
    fn base_url_works() {
        let mut u = BaseUrl::default();

        assert!(u.get().is_none());
        assert_eq!(u.get_ptr(), ptr::null());

        u.set(Url::parse("file:///example.txt").unwrap());

        assert_eq!(u.get().unwrap().as_str(), "file:///example.txt");

        unsafe {
            let p = u.get_ptr();
            let cstr = CStr::from_ptr(p);
            assert_eq!(cstr.to_str().unwrap(), "file:///example.txt");
        }
    }
}
