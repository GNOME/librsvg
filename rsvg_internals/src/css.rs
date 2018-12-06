use std::collections::hash_map::Entry;
use std::collections::HashMap;
use std::ptr;
use std::slice;
use std::str::{self, FromStr};

use libc;

use glib::translate::*;
use glib_sys::{gboolean, gpointer, GList};

use attributes::Attribute;
use croco::*;
use handle::{self, RsvgHandle};
use state::State;
use util::utf8_cstr;

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

struct DocHandlerData {
    handle: *mut RsvgHandle,
    selector: *mut CRSelector,
}

fn parse_into_handle(handle: *mut RsvgHandle, buf: &str) {
    unsafe {
        let handler_data = DocHandlerData {
            handle,
            selector: ptr::null_mut(),
        };

        let doc_handler = cr_doc_handler_new();
        init_cr_doc_handler(&mut *doc_handler);

        (*doc_handler).app_data = &handler_data as *const _ as gpointer;

        let buf_ptr = buf.as_ptr() as *mut _;
        let buf_len = buf.len() as libc::c_ulong;

        let parser = cr_parser_new_from_buf(buf_ptr, buf_len, CR_UTF_8, false.to_glib());
        if parser.is_null() {
            cr_doc_handler_unref(doc_handler);
            return;
        }

        cr_parser_set_sac_handler(parser, doc_handler);
        cr_doc_handler_unref(doc_handler);

        cr_parser_set_use_core_grammar(parser, false.to_glib());
        cr_parser_parse(parser);

        cr_parser_destroy(parser);
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
    _a_media_list: *mut GList,
    a_uri: CRString,
    _a_uri_default_ns: CRString,
    _a_location: CRParsingLocation,
) {
    let handler_data = get_doc_handler_data(a_this);

    if a_uri.is_null() {
        return;
    }

    let raw_uri = cr_string_peek_raw_str(a_uri);
    let uri = utf8_cstr(raw_uri);

    if let Ok(binary_data) = handle::acquire_data(handler_data.handle, uri) {
        if binary_data.content_type.as_ref().map(String::as_ref) == Some("text/css") {
            parse_into_handle(
                handler_data.handle,
                str::from_utf8_unchecked(&binary_data.data),
            );
        }
    }
}

unsafe fn get_doc_handler_data<'a>(doc_handler: *mut CRDocHandler) -> &'a mut DocHandlerData {
    &mut *((*doc_handler).app_data as *mut DocHandlerData)
}

unsafe extern "C" fn css_start_selector(
    a_this: *mut CRDocHandler,
    a_selector_list: *mut CRSelector,
) {
    let handler_data = get_doc_handler_data(a_this);

    cr_selector_ref(a_selector_list);
    handler_data.selector = a_selector_list;
}

unsafe extern "C" fn css_end_selector(
    a_this: *mut CRDocHandler,
    _a_selector_list: *mut CRSelector,
) {
    let handler_data = get_doc_handler_data(a_this);

    cr_selector_unref(handler_data.selector);
    handler_data.selector = ptr::null_mut();
}

unsafe extern "C" fn css_property(
    a_this: *mut CRDocHandler,
    a_name: CRString,
    a_expression: CRTerm,
    a_is_important: gboolean,
) {
    let handler_data = get_doc_handler_data(a_this);

    if a_name.is_null() || a_expression.is_null() || handler_data.selector.is_null() {
        return;
    }

    let mut cur_sel = handler_data.selector;
    while !cur_sel.is_null() {
        let simple_sel = (*cur_sel).simple_sel;

        if !simple_sel.is_null() {
            let raw_selector_name = cr_simple_sel_to_string(simple_sel) as *mut libc::c_char;

            if !raw_selector_name.is_null() {
                let raw_prop_name = cr_string_peek_raw_str(a_name);
                let prop_name = utf8_cstr(raw_prop_name);

                let prop_value =
                    <String as FromGlibPtrFull<_>>::from_glib_full(cr_term_to_string(a_expression));

                let selector_name =
                    <String as FromGlibPtrFull<_>>::from_glib_full(raw_selector_name);

                let important = from_glib(a_is_important);

                let styles = handle::get_css_styles_mut(handler_data.handle);

                styles.define(&selector_name, prop_name, &prop_value, important);
            }
        }

        cur_sel = (*cur_sel).next;
    }
}

unsafe extern "C" fn css_error(_a_this: *mut CRDocHandler) {
    println!("CSS parsing error");
}

unsafe extern "C" fn css_unrecoverable_error(_a_this: *mut CRDocHandler) {
    println!("CSS unrecoverable error");
}

#[no_mangle]
pub extern "C" fn rsvg_css_styles_new() -> *mut CssStyles {
    Box::into_raw(Box::new(CssStyles::new()))
}

#[no_mangle]
pub unsafe extern "C" fn rsvg_css_styles_free(raw_styles: *mut CssStyles) {
    assert!(!raw_styles.is_null());
    Box::from_raw(raw_styles);
}

#[no_mangle]
pub unsafe extern "C" fn rsvg_css_parse_into_handle(
    handle: *mut RsvgHandle,
    buf: *const libc::c_char,
    len: usize,
) {
    assert!(!handle.is_null());

    if buf.is_null() || len == 0 {
        return;
    }

    let bytes = slice::from_raw_parts(buf as *const u8, len);
    let utf8 = str::from_utf8_unchecked(bytes);

    parse_into_handle(handle, utf8);
}
