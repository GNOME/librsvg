use std::collections::hash_map::Entry;
use std::collections::HashMap;
use std::str::FromStr;

use libc;

use glib::translate::*;
use glib_sys::{self, gboolean, GList};

use attributes::Attribute;
use croco::*;
use state::State;
use util::utf8_cstr;

pub enum RsvgCssStyles {}

struct Declaration {
    prop_value: String,
    important: bool,
}

// Maps property_name -> Declaration
type DeclarationList = HashMap<String, Declaration>;

pub struct CssStyles {
    selectors_to_declarations: HashMap<String, DeclarationList>,
}

impl CssStyles {
    fn new() -> CssStyles {
        CssStyles {
            selectors_to_declarations: HashMap::new(),
        }
    }

    fn define(&mut self, selector: &str, prop_name: &str, prop_value: &str, important: bool) {
        let decl_list = self
            .selectors_to_declarations
            .entry(selector.to_string())
            .or_insert_with(|| DeclarationList::new());

        match decl_list.entry(prop_name.to_string()) {
            Entry::Occupied(mut e) => {
                let decl = e.get_mut();

                if !decl.important {
                    decl.prop_value = prop_value.to_string();
                    decl.important = important;
                }
            }

            Entry::Vacant(v) => {
                v.insert(Declaration {
                    prop_value: prop_value.to_string(),
                    important,
                });
            }
        }
    }

    pub fn lookup_apply(&self, selector: &str, state: &mut State) -> bool {
        if let Some(decl_list) = self.selectors_to_declarations.get(selector) {
            for (prop_name, declaration) in decl_list.iter() {
                if let Ok(attr) = Attribute::from_str(prop_name) {
                    // FIXME: this is ignoring errors
                    let _ = state.parse_style_pair(
                        attr,
                        &declaration.prop_value,
                        declaration.important,
                    );
                }
            }

            true
        } else {
            false
        }
    }
}

fn init_cr_doc_handler(handler: &mut CRDocHandler) {
    handler.import_style = Some(css_import_style);
    handler.start_selector = Some(css_start_selector);
    handler.end_selector = Some(css_end_selector);
    handler.property = Some(css_property);
    handler.error = Some(css_error);
    handler.unrecoverable_error = Some(css_unrecoverable_error);
}

unsafe extern "C" fn css_import_style(
    a_this: *mut CRDocHandler,
    a_media_list: *mut GList,
    a_uri: CRString,
    a_uri_default_ns: CRString,
    a_location: CRParsingLocation,
) {
    unimplemented!();
}

unsafe extern "C" fn css_start_selector(
    a_this: *mut CRDocHandler,
    a_selector_list: *mut CRSelector,
) {
    unimplemented!();
}

unsafe extern "C" fn css_end_selector(a_this: *mut CRDocHandler, a_selector_list: *mut CRSelector) {
    unimplemented!();
}

unsafe extern "C" fn css_property(
    a_this: *mut CRDocHandler,
    a_name: CRString,
    a_expression: CRTerm,
    a_is_important: gboolean,
) {
    unimplemented!();
}

unsafe extern "C" fn css_error(a_this: *mut CRDocHandler) {
    unimplemented!();
}

unsafe extern "C" fn css_unrecoverable_error(a_this: *mut CRDocHandler) {
    unimplemented!();
}

#[no_mangle]
pub extern "C" fn rsvg_css_styles_new() -> *mut RsvgCssStyles {
    Box::into_raw(Box::new(CssStyles::new())) as *mut RsvgCssStyles
}

#[no_mangle]
pub unsafe extern "C" fn rsvg_css_styles_free(raw_styles: *mut RsvgCssStyles) {
    assert!(!raw_styles.is_null());

    Box::from_raw(raw_styles as *mut CssStyles);
}

#[no_mangle]
pub extern "C" fn rsvg_css_styles_define(
    raw_styles: *mut RsvgCssStyles,
    selector: *const libc::c_char,
    prop_name: *const libc::c_char,
    prop_value: *const libc::c_char,
    important: glib_sys::gboolean,
) {
    assert!(!raw_styles.is_null());
    assert!(!selector.is_null());
    assert!(!prop_name.is_null());
    assert!(!prop_value.is_null());

    let styles = unsafe { &mut *(raw_styles as *mut CssStyles) };
    let selector = unsafe { utf8_cstr(selector) };
    let prop_name = unsafe { utf8_cstr(prop_name) };
    let prop_value = unsafe { utf8_cstr(prop_value) };

    styles.define(selector, prop_name, prop_value, from_glib(important));
}
