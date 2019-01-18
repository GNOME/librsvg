use std::collections::hash_map::Entry;
use std::collections::{HashMap, HashSet};
use std::ptr;
use std::str::{self, FromStr};

use libc;
use url::Url;

use glib::translate::*;
use glib_sys::{gboolean, gpointer, GList};

use allowed_url::AllowedUrl;
use attributes::Attribute;
use croco::*;
use error::LoadingError;
use io::{self, BinaryData};
use properties::SpecifiedValues;
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
    pub fn new() -> CssStyles {
        CssStyles {
            selectors_to_declarations: HashMap::new(),
        }
    }

    pub fn parse(&mut self, base_url: Option<&Url>, buf: &str) {
        if buf.len() == 0 {
            return; // libcroco doesn't like empty strings :(
        }

        unsafe {
            let mut handler_data = DocHandlerData {
                base_url,
                css_styles: self,
                selector: ptr::null_mut(),
            };

            let doc_handler = cr_doc_handler_new();
            init_cr_doc_handler(&mut *doc_handler);

            (*doc_handler).app_data = &mut handler_data as *mut _ as gpointer;

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

    pub fn load_css(&mut self, aurl: &AllowedUrl) -> Result<(), LoadingError> {
        io::acquire_data(aurl, None)
            .and_then(|data| {
                let BinaryData {
                    data: bytes,
                    content_type,
                } = data;

                if content_type.as_ref().map(String::as_ref) == Some("text/css") {
                    Ok(bytes)
                } else {
                    rsvg_log!("\"{}\" is not of type text/css; ignoring", aurl);
                    Err(LoadingError::BadCss)
                }
            })
            .and_then(|bytes| {
                String::from_utf8(bytes).map_err(|_| {
                    rsvg_log!(
                        "\"{}\" does not contain valid UTF-8 CSS data; ignoring",
                        aurl
                    );
                    LoadingError::BadCss
                })
            })
            .and_then(|utf8| {
                self.parse(Some(&aurl), &utf8);
                Ok(()) // FIXME: return CSS parsing errors
            })
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

    pub fn lookup_apply(
        &self,
        selector: &str,
        values: &mut SpecifiedValues,
        important_styles: &mut HashSet<Attribute>,
    ) -> bool {
        if let Some(decl_list) = self.selectors_to_declarations.get(selector) {
            for (prop_name, declaration) in decl_list.iter() {
                if let Ok(attr) = Attribute::from_str(prop_name) {
                    // FIXME: this is ignoring errors
                    let _ = values.parse_style_pair(
                        attr,
                        &declaration.prop_value,
                        declaration.important,
                        important_styles,
                    );
                }
            }

            true
        } else {
            false
        }
    }
}

struct DocHandlerData<'a> {
    base_url: Option<&'a Url>,
    css_styles: &'a mut CssStyles,
    selector: *mut CRSelector,
}

macro_rules! get_doc_handler_data {
    ($doc_handler:expr) => {
        &mut *((*$doc_handler).app_data as *mut DocHandlerData)
    };
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
    let handler_data = get_doc_handler_data!(a_this);

    if a_uri.is_null() {
        return;
    }

    let raw_uri = cr_string_peek_raw_str(a_uri);
    let uri = utf8_cstr(raw_uri);

    if let Ok(aurl) = AllowedUrl::from_href(uri, handler_data.base_url) {
        // FIXME: handle CSS errors
        let _ = handler_data.css_styles.load_css(&aurl);
    } else {
        rsvg_log!("disallowed URL \"{}\" for importing CSS", uri);
    }
}

unsafe extern "C" fn css_start_selector(
    a_this: *mut CRDocHandler,
    a_selector_list: *mut CRSelector,
) {
    let handler_data = get_doc_handler_data!(a_this);

    cr_selector_ref(a_selector_list);
    handler_data.selector = a_selector_list;
}

unsafe extern "C" fn css_end_selector(
    a_this: *mut CRDocHandler,
    _a_selector_list: *mut CRSelector,
) {
    let handler_data = get_doc_handler_data!(a_this);

    cr_selector_unref(handler_data.selector);
    handler_data.selector = ptr::null_mut();
}

unsafe extern "C" fn css_property(
    a_this: *mut CRDocHandler,
    a_name: CRString,
    a_expression: CRTerm,
    a_is_important: gboolean,
) {
    let handler_data = get_doc_handler_data!(a_this);

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

                handler_data
                    .css_styles
                    .define(&selector_name, prop_name, &prop_value, important);
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
