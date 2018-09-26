// This is a hand-written binding to a very minimal part of libcroco.  We don't use bindgen because
// it wants to import pretty much all of glib's types and functions, and we just need a few.

use glib_sys::{gboolean, gpointer, GList};
use libc;

// Opaque types from libcroco, or those which we only manipulate through libcroco functions
pub type CRString = gpointer;
pub type CRSimpleSel = gpointer;
pub type CRParser = gpointer;
pub type CRTerm = gpointer;

pub type CRStatus = u32;

pub type CREncoding = u32;
pub const CR_UTF_8: CREncoding = 5;

#[repr(C)]
pub struct CRParsingLocation {
    pub line: libc::c_uint,
    pub column: libc::c_uint,
    pub byte_offset: libc::c_uint,
}

#[repr(C)]
pub struct CRSelector {
    pub simple_sel: CRSimpleSel,

    pub next: *mut CRSelector,
    pub prev: *mut CRSelector,

    pub location: CRParsingLocation,
    pub ref_count: libc::c_long,
}

#[repr(C)]
pub struct CRDocHandler {
    pub priv_: gpointer,

    pub app_data: gpointer,

    pub start_document: gpointer,
    pub end_document: gpointer,
    pub charset: gpointer,

    pub import_style: Option<
        unsafe extern "C" fn(
            a_this: *mut CRDocHandler,
            a_media_list: *mut GList,
            a_uri: CRString,
            a_uri_default_ns: CRString,
            a_location: CRParsingLocation,
        ),
    >,

    pub import_style_result: gpointer,
    pub namespace_declaration: gpointer,
    pub comment: gpointer,

    pub start_selector:
        Option<unsafe extern "C" fn(a_this: *mut CRDocHandler, a_selector_list: *mut CRSelector)>,

    pub end_selector:
        Option<unsafe extern "C" fn(a_this: *mut CRDocHandler, a_selector_list: *mut CRSelector)>,

    pub property: Option<
        unsafe extern "C" fn(
            a_this: *mut CRDocHandler,
            a_name: CRString,
            a_expression: CRTerm,
            a_is_important: gboolean,
        ),
    >,

    pub start_font_face: gpointer,
    pub end_font_face: gpointer,
    pub start_media: gpointer,
    pub end_media: gpointer,
    pub start_page: gpointer,
    pub end_page: gpointer,
    pub ignorable_at_rule: gpointer,

    pub error: Option<unsafe extern "C" fn(a_this: *mut CRDocHandler)>,
    pub unrecoverable_error: Option<unsafe extern "C" fn(a_this: *mut CRDocHandler)>,

    pub resolve_import: gboolean,
    pub ref_count: libc::c_ulong,
}

extern "C" {
    pub fn cr_selector_ref(a_this: *mut CRSelector);
    pub fn cr_selector_unref(a_this: *mut CRSelector) -> gboolean;

    pub fn cr_simple_sel_to_string(a_this: CRSimpleSel) -> *mut libc::c_char;

    pub fn cr_string_peek_raw_str(a_this: CRString) -> *const libc::c_char;

    pub fn cr_term_to_string(a_this: CRTerm) -> *mut libc::c_char;

    pub fn cr_doc_handler_new() -> *mut CRDocHandler;
    pub fn cr_doc_handler_unref(a_this: *mut CRDocHandler) -> gboolean;

    pub fn cr_parser_new_from_buf(
        a_buf: *mut libc::c_char,
        a_len: libc::c_ulong,
        a_enc: CREncoding,
        a_free_buf: gboolean,
    ) -> CRParser;

    pub fn cr_parser_set_sac_handler(a_this: CRParser, a_handler: *mut CRDocHandler) -> CRStatus;

    pub fn cr_parser_set_use_core_grammar(
        a_this: CRParser,
        a_use_core_grammar: gboolean,
    ) -> CRStatus;

    pub fn cr_parser_parse(a_this: CRParser) -> CRStatus;
    pub fn cr_parser_destroy(a_this: CRParser);

}
