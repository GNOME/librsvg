// This file provides functions to create a libxml2 xmlParserCtxtPtr, configured
// to read from a gio::InputStream, and to maintain its loading data in an XmlState.

use gio;
use gio::prelude::*;
use gio_sys;
use glib_sys;
use std::mem;
use std::ptr;
use std::slice;
use std::str;

use glib::translate::*;

use error::set_gerror;
use property_bag::PropertyBag;
use util::utf8_cstr;
use xml::XmlState;
use xml2::*;

extern "C" {
    fn rsvg_sax_error_cb(data: *mut libc::c_void);
}

fn get_xml2_sax_handler() -> xmlSAXHandler {
    let mut h: xmlSAXHandler = unsafe { mem::zeroed() };

    h.getEntity = Some(sax_get_entity_cb);
    h.entityDecl = Some(sax_entity_decl_cb);
    h.unparsedEntityDecl = Some(sax_unparsed_entity_decl_cb);
    h.getParameterEntity = Some(sax_get_parameter_entity_cb);
    h.characters = Some(sax_characters_cb);
    h.cdataBlock = Some(sax_characters_cb);
    h.startElement = Some(sax_start_element_cb);
    h.endElement = Some(sax_end_element_cb);
    h.processingInstruction = Some(sax_processing_instruction_cb);

    // This one is defined in the C code, because the prototype has varargs
    // and we can't handle those from Rust :(
    h.error = rsvg_sax_error_cb as *mut _;

    h
}

fn free_xml_parser_and_doc(parser: xmlParserCtxtPtr) {
    unsafe {
        if !parser.is_null() {
            let rparser = &mut *parser;

            if !rparser.myDoc.is_null() {
                xmlFreeDoc(rparser.myDoc);
                rparser.myDoc = ptr::null_mut();
            }

            xmlFreeParserCtxt(parser);
        }
    }
}

unsafe extern "C" fn sax_get_entity_cb(
    ctx: *mut libc::c_void,
    name: *const libc::c_char,
) -> xmlEntityPtr {
    let xml = &*(ctx as *mut XmlState);

    assert!(!name.is_null());
    let name = utf8_cstr(name);

    xml.entity_lookup(name).unwrap_or(ptr::null_mut())
}

unsafe extern "C" fn sax_entity_decl_cb(
    ctx: *mut libc::c_void,
    name: *const libc::c_char,
    type_: libc::c_int,
    _public_id: *const libc::c_char,
    _system_id: *const libc::c_char,
    content: *const libc::c_char,
) {
    let xml = &mut *(ctx as *mut XmlState);

    assert!(!name.is_null());

    if type_ != XML_INTERNAL_GENERAL_ENTITY {
        // We don't allow loading external entities; we don't support
        // defining parameter entities in the DTD, and libxml2 should
        // handle internal predefined entities by itself (e.g. "&amp;").
        return;
    }

    let entity = xmlNewEntity(
        ptr::null_mut(),
        name,
        type_,
        ptr::null(),
        ptr::null(),
        content,
    );
    assert!(!entity.is_null());

    let name = utf8_cstr(name);
    xml.entity_insert(name, entity);
}

unsafe extern "C" fn sax_unparsed_entity_decl_cb(
    ctx: *mut libc::c_void,
    name: *const libc::c_char,
    public_id: *const libc::c_char,
    system_id: *const libc::c_char,
    _notation_name: *const libc::c_char,
) {
    sax_entity_decl_cb(
        ctx,
        name,
        XML_INTERNAL_GENERAL_ENTITY,
        public_id,
        system_id,
        ptr::null(),
    );
}

unsafe extern "C" fn sax_start_element_cb(
    ctx: *mut libc::c_void,
    name: *const libc::c_char,
    atts: *const *const libc::c_char,
) {
    let xml = &mut *(ctx as *mut XmlState);

    assert!(!name.is_null());
    let name = utf8_cstr(name);

    let pbag = PropertyBag::new_from_key_value_pairs(atts);

    xml.start_element(name, &pbag);
}

unsafe extern "C" fn sax_end_element_cb(ctx: *mut libc::c_void, name: *const libc::c_char) {
    let xml = &mut *(ctx as *mut XmlState);

    assert!(!name.is_null());
    let name = utf8_cstr(name);

    xml.end_element(name);
}

unsafe extern "C" fn sax_characters_cb(
    ctx: *mut libc::c_void,
    unterminated_text: *const libc::c_char,
    len: libc::c_int,
) {
    let xml = &mut *(ctx as *mut XmlState);

    assert!(!unterminated_text.is_null());
    assert!(len >= 0);

    // libxml2 already validated the incoming string as UTF-8.  Note that
    // it is *not* nul-terminated; this is why we create a byte slice first.
    let bytes = std::slice::from_raw_parts(unterminated_text as *const u8, len as usize);
    let utf8 = str::from_utf8_unchecked(bytes);

    xml.characters(utf8);
}

unsafe extern "C" fn sax_processing_instruction_cb(
    ctx: *mut libc::c_void,
    target: *const libc::c_char,
    data: *const libc::c_char,
) {
    let xml = &mut *(ctx as *mut XmlState);

    assert!(!target.is_null());
    let target = utf8_cstr(target);

    assert!(!data.is_null());
    let data = utf8_cstr(data);

    xml.processing_instruction(target, data);
}

unsafe extern "C" fn sax_get_parameter_entity_cb(
    ctx: *mut libc::c_void,
    name: *const libc::c_char,
) -> xmlEntityPtr {
    sax_get_entity_cb(ctx, name)
}

fn set_xml_parse_options(parser: xmlParserCtxtPtr, unlimited_size: bool) {
    let mut options: libc::c_int = XML_PARSE_NONET | XML_PARSE_BIG_LINES;

    if unlimited_size {
        options |= XML_PARSE_HUGE;
    }

    unsafe {
        xmlCtxtUseOptions(parser, options);

        // If false, external entities work, but internal ones don't. if
        // true, internal entities work, but external ones don't. favor
        // internal entities, in order to not cause a regression
        (*parser).replaceEntities = 1;
    }
}

// Struct used as closure data for xmlCreateIOParserCtxt().  In conjunction
// with stream_ctx_read() and stream_ctx_close(), this struct provides the
// I/O callbacks and their context for libxml2.
//
// We call I/O methods on the stream, and as soon as we get an error
// we store it in the gio_error field.  Libxml2 just allows us to
// return -1 from the I/O callbacks in that case; it doesn't actually
// see the error code.
//
// The gio_error field comes from the place that constructs the
// StreamCtx.  That place is later responsible for seeing if the error
// is set; if it is, it means that there was an I/O error.  Otherwise,
// there were no I/O errors but the caller must then ask libxml2 for
// XML parsing errors.
struct StreamCtx {
    stream: gio::InputStream,
    cancellable: Option<gio::Cancellable>,
    gio_error: *mut *mut glib_sys::GError,
}

// read() callback from xmlCreateIOParserCtxt()
unsafe extern "C" fn stream_ctx_read(
    context: *mut libc::c_void,
    buffer: *mut libc::c_char,
    len: libc::c_int,
) -> libc::c_int {
    let ctx = &mut *(context as *mut StreamCtx);

    // has the error been set already?
    if !(*ctx.gio_error).is_null() {
        return -1;
    }

    let buf: &mut [u8] = slice::from_raw_parts_mut(buffer as *mut u8, len as usize);

    match ctx.stream.read(buf, ctx.cancellable.as_ref()) {
        Ok(size) => size as libc::c_int,

        Err(e) => {
            let e: *const glib_sys::GError = e.to_glib_full();
            *ctx.gio_error = e as *mut _;
            -1
        }
    }
}

// close() callback from xmlCreateIOParserCtxt()
unsafe extern "C" fn stream_ctx_close(context: *mut libc::c_void) -> libc::c_int {
    let ctx = &mut *(context as *mut StreamCtx);

    let ret = match ctx.stream.close(ctx.cancellable.as_ref()) {
        Ok(()) => 0,

        Err(e) => {
            // don't overwrite a previous error
            if (*ctx.gio_error).is_null() {
                let e: *const glib_sys::GError = e.to_glib_full();
                *ctx.gio_error = e as *mut _;
            }

            -1
        }
    };

    Box::from_raw(ctx);

    ret
}

fn create_xml_stream_parser(
    xml: &mut XmlState,
    unlimited_size: bool,
    stream: gio::InputStream,
    cancellable: Option<gio::Cancellable>,
    gio_error: *mut *mut glib_sys::GError,
) -> Result<xmlParserCtxtPtr, ParseFromStreamError> {
    let ctx = Box::new(StreamCtx {
        stream,
        cancellable,
        gio_error,
    });

    let mut sax_handler = get_xml2_sax_handler();

    unsafe {
        let parser = xmlCreateIOParserCtxt(
            &mut sax_handler,
            xml as *mut _ as *mut _,
            Some(stream_ctx_read),
            Some(stream_ctx_close),
            Box::into_raw(ctx) as *mut _,
            XML_CHAR_ENCODING_NONE,
        );

        if parser.is_null() {
            // on error, xmlCreateIOParserCtxt() frees our ctx via the
            // stream_ctx_close function
            Err(ParseFromStreamError::CouldNotCreateParser)
        } else {
            set_xml_parse_options(parser, unlimited_size);
            Ok(parser)
        }
    }
}

#[no_mangle]
pub unsafe extern "C" fn rsvg_create_xml_push_parser(
    xml: *mut XmlState,
    unlimited_size: glib_sys::gboolean,
    base_uri: *const libc::c_char,
    error: *mut *mut glib_sys::GError,
) -> xmlParserCtxtPtr {
    let mut sax_handler = get_xml2_sax_handler();

    let parser = xmlCreatePushParserCtxt(&mut sax_handler, xml as *mut _, ptr::null(), 0, base_uri);

    if parser.is_null() {
        set_gerror(error, 0, "Error creating XML parser");
    } else {
        set_xml_parse_options(parser, from_glib(unlimited_size));
    }

    parser
}

fn xml2_error_to_string(xerr: xmlErrorPtr) -> String {
    unsafe {
        if !xerr.is_null() {
            let xerr = &*xerr;

            let file = if xerr.file.is_null() {
                "data".to_string()
            } else {
                from_glib_none(xerr.file)
            };

            let message = if xerr.message.is_null() {
                "-".to_string()
            } else {
                from_glib_none(xerr.message)
            };

            format!(
                "Error domain {} code {} on line {} column {} of {}: {}",
                xerr.domain, xerr.code, xerr.line, xerr.int2, file, message
            )
        } else {
            // The error is not set?  Return a generic message :(
            "Error parsing XML data".to_string()
        }
    }
}

#[no_mangle]
pub unsafe extern "C" fn rsvg_set_error_from_xml(
    error: *mut *mut glib_sys::GError,
    ctxt: xmlParserCtxtPtr,
) {
    let xerr = xmlCtxtGetLastError(ctxt as *mut _);

    set_gerror(error, 0, &xml2_error_to_string(xerr));
}

// Error returned when parsing an XML stream
pub enum ParseFromStreamError {
    // We couldn't even create the libxml2 parser
    CouldNotCreateParser,

    // GIO error from the I/O callbacks
    IoError(glib::Error),

    // XML parsing error from libxml2
    XmlParseError(String),
}

// Parses XML from a stream into an XmlState.
//
// This can be called "in the middle" of an XmlState's processing status,
// for example, when including another XML file via xi:include.
pub fn xml_state_parse_from_stream(
    xml: &mut XmlState,
    unlimited_size: bool,
    stream: gio::InputStream,
    cancellable: Option<gio::Cancellable>,
) -> Result<(), ParseFromStreamError> {
    let mut gio_err: *mut glib_sys::GError = ptr::null_mut();

    match create_xml_stream_parser(xml, unlimited_size, stream, cancellable, &mut gio_err) {
        Ok(parser) => unsafe {
            let xml_parse_success = xmlParseDocument(parser) == 0;

            let io_success = gio_err.is_null();

            let res;

            if !io_success {
                res = Err(ParseFromStreamError::IoError(from_glib_full(gio_err)));
            } else if !xml_parse_success {
                let xerr = xmlCtxtGetLastError(parser as *mut _);
                res = Err(ParseFromStreamError::XmlParseError(xml2_error_to_string(
                    xerr,
                )));
            } else {
                res = Ok(());
            }

            free_xml_parser_and_doc(parser);

            res
        },

        Err(e) => Err(e),
    }
}

#[no_mangle]
pub unsafe extern "C" fn rsvg_xml_state_parse_from_stream(
    xml: *mut XmlState,
    unlimited_size: glib_sys::gboolean,
    stream: *mut gio_sys::GInputStream,
    cancellable: *mut gio_sys::GCancellable,
    error: *mut *mut glib_sys::GError,
) -> glib_sys::gboolean {
    assert!(!xml.is_null());
    let xml = &mut *xml;

    let unlimited_size = from_glib(unlimited_size);

    let stream = from_glib_none(stream);
    let cancellable = from_glib_none(cancellable);

    match xml_state_parse_from_stream(xml, unlimited_size, stream, cancellable) {
        Ok(()) => true.to_glib(),

        Err(e) => {
            match e {
                ParseFromStreamError::CouldNotCreateParser => {
                    set_gerror(error, 0, "Error creating XML parser");
                }

                ParseFromStreamError::IoError(e) => {
                    if !error.is_null() {
                        *error = e.to_glib_full() as *mut _;
                    }
                }

                ParseFromStreamError::XmlParseError(s) => {
                    set_gerror(error, 0, &s);
                }
            }

            false.to_glib()
        }
    }
}
