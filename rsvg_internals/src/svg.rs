use cairo::{ImageSurface, Status};
use gdk_pixbuf::{PixbufLoader, PixbufLoaderExt};
use gio;
use glib::translate::*;
use std::cell::RefCell;
use std::collections::hash_map::Entry;
use std::collections::HashMap;
use std::rc::Rc;

use allowed_url::{AllowedUrl, Fragment};
use error::LoadingError;
use handle::LoadOptions;
use io;
use node::RsvgNode;
use state::ComputedValues;
use surface_utils::shared_surface::SharedImageSurface;
use xml::XmlState;
use xml2_load::xml_state_load_from_possibly_compressed_stream;

/// A loaded SVG file and its derived data
///
/// This contains the tree of nodes (SVG elements), the mapping
/// of id to node, and the CSS styles defined for this SVG.
pub struct Svg {
    tree: RsvgNode,

    ids: HashMap<String, RsvgNode>,

    // These require interior mutability because we load the extern
    // resources all over the place.  Eventually we'll be able to do this
    // once, at loading time, and keep this immutable.
    externs: RefCell<Resources>,
    images: RefCell<Images>,

    // Once we do not need to load externs, we can drop this as well
    load_options: LoadOptions,
}

impl Svg {
    pub fn new(tree: RsvgNode, ids: HashMap<String, RsvgNode>, load_options: LoadOptions) -> Svg {
        let values = ComputedValues::default();
        tree.cascade(&values);

        Svg {
            tree,
            ids,
            externs: RefCell::new(Resources::new()),
            images: RefCell::new(Images::new()),
            load_options,
        }
    }

    pub fn load_from_stream(
        load_options: &LoadOptions,
        stream: &gio::InputStream,
        cancellable: Option<&gio::Cancellable>,
    ) -> Result<Svg, LoadingError> {
        let load_flags = load_options.flags;
        let mut xml = XmlState::new(load_options);

        xml_state_load_from_possibly_compressed_stream(&mut xml, load_flags, stream, cancellable)?;

        xml.steal_result()
    }

    pub fn root(&self) -> RsvgNode {
        self.tree.clone()
    }

    pub fn lookup(&self, fragment: &Fragment) -> Option<RsvgNode> {
        if fragment.uri().is_some() {
            self.externs
                .borrow_mut()
                .lookup(&self.load_options, fragment)
        } else {
            self.lookup_node_by_id(fragment.fragment())
        }
    }

    pub fn lookup_node_by_id(&self, id: &str) -> Option<RsvgNode> {
        self.ids.get(id).map(|n| (*n).clone())
    }

    pub fn lookup_image(&self, href: &str) -> Option<ImageSurface> {
        self.images
            .borrow_mut()
            .lookup(&self.load_options, href)
            .and_then(|s| s.into_image_surface().ok())
    }
}

struct Resources {
    resources: HashMap<AllowedUrl, Rc<Svg>>,
}

impl Resources {
    pub fn new() -> Resources {
        Resources {
            resources: Default::default(),
        }
    }

    pub fn lookup(&mut self, load_options: &LoadOptions, fragment: &Fragment) -> Option<RsvgNode> {
        if let Some(ref href) = fragment.uri() {
            // FIXME: propagate errors from the loader
            match self.get_extern_svg(load_options, href) {
                Ok(svg) => svg.lookup_node_by_id(fragment.fragment()),

                Err(()) => None,
            }
        } else {
            unreachable!();
        }
    }

    fn get_extern_svg(&mut self, load_options: &LoadOptions, href: &str) -> Result<Rc<Svg>, ()> {
        let aurl = AllowedUrl::from_href(href, load_options.base_url.as_ref()).map_err(|_| ())?;

        match self.resources.entry(aurl) {
            Entry::Occupied(e) => Ok(e.get().clone()),
            Entry::Vacant(e) => {
                // FIXME: propagate errors
                let svg = load_svg(load_options, e.key()).map_err(|_| ())?;
                let rc_svg = e.insert(Rc::new(svg));
                Ok(rc_svg.clone())
            }
        }
    }
}

struct Images {
    images: HashMap<AllowedUrl, SharedImageSurface>,
}

impl Images {
    pub fn new() -> Images {
        Images {
            images: Default::default(),
        }
    }

    pub fn lookup(&mut self, load_options: &LoadOptions, href: &str) -> Option<SharedImageSurface> {
        // FIXME: propagate errors
        let aurl = AllowedUrl::from_href(href, load_options.base_url.as_ref()).ok()?;

        match self.images.entry(aurl) {
            Entry::Occupied(e) => Some(e.get().clone()),
            Entry::Vacant(e) => {
                // FIXME: propagate errors
                let surface = load_image(load_options, e.key()).ok()?;
                let res = e.insert(surface);
                Some(res.clone())
            }
        }
    }
}

fn load_svg(load_options: &LoadOptions, aurl: &AllowedUrl) -> Result<Svg, LoadingError> {
    // FIXME: pass a cancellable to these
    io::acquire_stream(aurl, None).and_then(|stream| {
        Svg::load_from_stream(&load_options.copy_with_base_url(aurl), &stream, None)
    })
}

fn load_image(
    load_options: &LoadOptions,
    aurl: &AllowedUrl,
) -> Result<SharedImageSurface, LoadingError> {
    let data = io::acquire_data(&aurl, None)?;

    if data.data.len() == 0 {
        return Err(LoadingError::EmptyData);
    }

    let loader = if let Some(ref content_type) = data.content_type {
        PixbufLoader::new_with_mime_type(content_type)?
    } else {
        PixbufLoader::new()
    };

    loader.write(&data.data)?;
    loader.close()?;

    let pixbuf = loader.get_pixbuf().ok_or(LoadingError::Unknown)?;

    let surface = SharedImageSurface::from_pixbuf(&pixbuf)?;

    if load_options.flags.keep_image_data {
        if let Some(mime_type) = data.content_type {
            extern "C" {
                fn cairo_surface_set_mime_data(
                    surface: *mut cairo_sys::cairo_surface_t,
                    mime_type: *const libc::c_char,
                    data: *mut libc::c_char,
                    length: libc::c_ulong,
                    destroy: cairo_sys::cairo_destroy_func_t,
                    closure: *mut libc::c_void,
                ) -> Status;
            }

            let data_ptr = ToGlibContainerFromSlice::to_glib_full_from_slice(&data.data);

            unsafe {
                let status = cairo_surface_set_mime_data(
                    surface.to_glib_none().0,
                    mime_type.to_glib_none().0,
                    data_ptr as *mut _,
                    data.data.len() as libc::c_ulong,
                    Some(glib_sys::g_free),
                    data_ptr as *mut _,
                );

                if status != Status::Success {
                    return Err(LoadingError::Cairo(status));
                }
            }
        }
    }

    Ok(surface)
}
