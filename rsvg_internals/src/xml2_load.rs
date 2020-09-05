//! Glue between the libxml2 API and our xml parser module.
//!
//! This file provides functions to create a libxml2 xmlParserCtxtPtr, configured
//! to read from a gio::InputStream, and to maintain its loading data in an XmlState.

use gio::prelude::*;
use std::borrow::Cow;
use std::cell::{Cell, RefCell};
use std::ptr;
use std::rc::Rc;
use std::slice;
use std::str;
use std::sync::Once;

use glib::translate::*;
use markup5ever::{namespace_url, ns, LocalName, Namespace, Prefix, QualName};

use crate::attributes::Attributes;
use crate::error::LoadingError;
use crate::util::{cstr, opt_utf8_cstr, utf8_cstr};
use crate::xml::XmlState;
use crate::xml2::*;

#[rustfmt::skip]
fn get_xml2_sax_handler() -> xmlSAXHandler {
    xmlSAXHandler {
        // first the unused callbacks
        internalSubset:        None,
        isStandalone:          None,
        hasInternalSubset:     None,
        hasExternalSubset:     None,
        resolveEntity:         None,
        notationDecl:          None,
        attributeDecl:         None,
        elementDecl:           None,
        setDocumentLocator:    None,
        startDocument:         None,
        endDocument:           None,
        reference:             None,
        ignorableWhitespace:   None,
        comment:               None,
        warning:               None,
        error:                 None,
        fatalError:            None,
        externalSubset:        None,

        _private:              ptr::null_mut(),

        // then the used callbacks
        getEntity:             Some(sax_get_entity_cb),
        entityDecl:            Some(sax_entity_decl_cb),
        unparsedEntityDecl:    Some(sax_unparsed_entity_decl_cb),
        getParameterEntity:    Some(sax_get_parameter_entity_cb),
        characters:            Some(sax_characters_cb),
        cdataBlock:            Some(sax_characters_cb),
        startElement:          None,
        endElement:            None,
        processingInstruction: Some(sax_processing_instruction_cb),
        startElementNs:        Some(sax_start_element_ns_cb),
        endElementNs:          Some(sax_end_element_ns_cb),
        serror:                Some(rsvg_sax_serror_cb),

        initialized:           XML_SAX2_MAGIC,
    }
}

unsafe extern "C" fn rsvg_sax_serror_cb(user_data: *mut libc::c_void, error: xmlErrorPtr) {
    let xml2_parser = &*(user_data as *mut Xml2Parser);
    let error = error.as_ref().unwrap();

    let level_name = match error.level {
        1 => "warning",
        2 => "error",
        3 => "fatal error",
        _ => "unknown error",
    };

    // "int2" is the column number
    let column = if error.int2 > 0 {
        Cow::Owned(format!(":{}", error.int2))
    } else {
        Cow::Borrowed("")
    };

    let full_error_message = format!(
        "{} code={} ({}) in {}:{}{}: {}",
        level_name,
        error.code,
        error.domain,
        cstr(error.file),
        error.line,
        column,
        cstr(error.message)
    );
    xml2_parser
        .state
        .error(LoadingError::XmlParseError(full_error_message));
}

fn free_xml_parser_and_doc(parser: xmlParserCtxtPtr) {
    // Free the ctxt and its ctxt->myDoc - libxml2 doesn't free them together
    // http://xmlsoft.org/html/libxml-parser.html#xmlFreeParserCtxt
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
    user_data: *mut libc::c_void,
    name: *const libc::c_char,
) -> xmlEntityPtr {
    let xml2_parser = &*(user_data as *mut Xml2Parser);

    assert!(!name.is_null());
    let name = utf8_cstr(name);

    xml2_parser
        .state
        .entity_lookup(name)
        .unwrap_or(ptr::null_mut())
}

unsafe extern "C" fn sax_entity_decl_cb(
    user_data: *mut libc::c_void,
    name: *const libc::c_char,
    type_: libc::c_int,
    _public_id: *const libc::c_char,
    _system_id: *const libc::c_char,
    content: *const libc::c_char,
) {
    let xml2_parser = &*(user_data as *mut Xml2Parser);

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
    xml2_parser.state.entity_insert(name, entity);
}

unsafe extern "C" fn sax_unparsed_entity_decl_cb(
    user_data: *mut libc::c_void,
    name: *const libc::c_char,
    public_id: *const libc::c_char,
    system_id: *const libc::c_char,
    _notation_name: *const libc::c_char,
) {
    sax_entity_decl_cb(
        user_data,
        name,
        XML_INTERNAL_GENERAL_ENTITY,
        public_id,
        system_id,
        ptr::null(),
    );
}

fn make_qual_name(prefix: Option<&str>, uri: Option<&str>, localname: &str) -> QualName {
    // FIXME: If the element doesn't have a namespace URI, we are falling back
    // to the SVG namespace.  In reality we need to take namespace scoping into account,
    // i.e. handle the "default namespace" active at that point in the XML stack.
    let element_ns = uri.map(Namespace::from).unwrap_or_else(|| ns!(svg));

    QualName::new(
        prefix.map(Prefix::from),
        element_ns,
        LocalName::from(localname),
    )
}

unsafe extern "C" fn sax_start_element_ns_cb(
    user_data: *mut libc::c_void,
    localname: *mut libc::c_char,
    prefix: *mut libc::c_char,
    uri: *mut libc::c_char,
    _nb_namespaces: libc::c_int,
    _namespaces: *mut *mut libc::c_char,
    nb_attributes: libc::c_int,
    _nb_defaulted: libc::c_int,
    attributes: *mut *mut libc::c_char,
) {
    let xml2_parser = &*(user_data as *mut Xml2Parser);

    assert!(!localname.is_null());

    let prefix = opt_utf8_cstr(prefix);
    let uri = opt_utf8_cstr(uri);
    let localname = utf8_cstr(localname);

    let qual_name = make_qual_name(prefix, uri, localname);

    let nb_attributes = nb_attributes as usize;
    let attrs = Attributes::new_from_xml2_attributes(nb_attributes, attributes as *const *const _);

    if let Err(e) = xml2_parser.state.start_element(qual_name, attrs) {
        let _: () = e; // guard in case we change the error type later

        let parser = xml2_parser.parser.get();
        xmlStopParser(parser);
    }
}

unsafe extern "C" fn sax_end_element_ns_cb(
    user_data: *mut libc::c_void,
    localname: *mut libc::c_char,
    prefix: *mut libc::c_char,
    uri: *mut libc::c_char,
) {
    let xml2_parser = &*(user_data as *mut Xml2Parser);

    assert!(!localname.is_null());

    let prefix = opt_utf8_cstr(prefix);
    let uri = opt_utf8_cstr(uri);
    let localname = utf8_cstr(localname);

    let qual_name = make_qual_name(prefix, uri, localname);

    xml2_parser.state.end_element(qual_name);
}

unsafe extern "C" fn sax_characters_cb(
    user_data: *mut libc::c_void,
    unterminated_text: *const libc::c_char,
    len: libc::c_int,
) {
    let xml2_parser = &*(user_data as *mut Xml2Parser);

    assert!(!unterminated_text.is_null());
    assert!(len >= 0);

    // libxml2 already validated the incoming string as UTF-8.  Note that
    // it is *not* nul-terminated; this is why we create a byte slice first.
    let bytes = std::slice::from_raw_parts(unterminated_text as *const u8, len as usize);
    let utf8 = str::from_utf8_unchecked(bytes);

    xml2_parser.state.characters(utf8);
}

unsafe extern "C" fn sax_processing_instruction_cb(
    user_data: *mut libc::c_void,
    target: *const libc::c_char,
    data: *const libc::c_char,
) {
    let xml2_parser = &*(user_data as *mut Xml2Parser);

    assert!(!target.is_null());
    let target = utf8_cstr(target);

    let data = if data.is_null() { "" } else { utf8_cstr(data) };

    xml2_parser.state.processing_instruction(target, data);
}

unsafe extern "C" fn sax_get_parameter_entity_cb(
    user_data: *mut libc::c_void,
    name: *const libc::c_char,
) -> xmlEntityPtr {
    sax_get_entity_cb(user_data, name)
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
    gio_error: Rc<RefCell<Option<glib::Error>>>,
}

// read() callback from xmlCreateIOParserCtxt()
unsafe extern "C" fn stream_ctx_read(
    context: *mut libc::c_void,
    buffer: *mut libc::c_char,
    len: libc::c_int,
) -> libc::c_int {
    let ctx = &mut *(context as *mut StreamCtx);

    let mut err_ref = ctx.gio_error.borrow_mut();

    // has the error been set already?
    if err_ref.is_some() {
        return -1;
    }

    let buf: &mut [u8] = slice::from_raw_parts_mut(buffer as *mut u8, len as usize);

    match ctx.stream.read(buf, ctx.cancellable.as_ref()) {
        Ok(size) => size as libc::c_int,

        Err(e) => {
            // Just store the first I/O error we get; ignore subsequent ones.
            *err_ref = Some(e);
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
            let mut err_ref = ctx.gio_error.borrow_mut();

            // don't overwrite a previous error
            if err_ref.is_none() {
                *err_ref = Some(e);
            }

            -1
        }
    };

    Box::from_raw(ctx);

    ret
}

fn init_libxml2() {
    static ONCE: Once = Once::new();

    ONCE.call_once(|| unsafe {
        xmlInitParser();
    });
}

pub struct Xml2Parser {
    parser: Cell<xmlParserCtxtPtr>,
    state: Rc<XmlState>,
    gio_error: Rc<RefCell<Option<glib::Error>>>,
}

impl Xml2Parser {
    pub fn from_stream(
        state: Rc<XmlState>,
        unlimited_size: bool,
        stream: &gio::InputStream,
        cancellable: Option<&gio::Cancellable>,
    ) -> Result<Box<Xml2Parser>, LoadingError> {
        init_libxml2();

        // The Xml2Parser we end up creating, if
        // xmlCreateIOParserCtxt() is successful, needs to hold a
        // location to place a GError from within the I/O callbacks
        // stream_ctx_read() and stream_ctx_close().  We put this
        // location in an Rc so that it can outlive the call to
        // xmlCreateIOParserCtxt() in case that fails, since on
        // failure that function frees the StreamCtx.
        let gio_error = Rc::new(RefCell::new(None));

        let ctx = Box::new(StreamCtx {
            stream: stream.clone(),
            cancellable: cancellable.cloned(),
            gio_error: gio_error.clone(),
        });

        let mut sax_handler = get_xml2_sax_handler();

        let mut xml2_parser = Box::new(Xml2Parser {
            parser: Cell::new(ptr::null_mut()),
            state,
            gio_error,
        });

        unsafe {
            let parser = xmlCreateIOParserCtxt(
                &mut sax_handler,
                xml2_parser.as_mut() as *mut _ as *mut _,
                Some(stream_ctx_read),
                Some(stream_ctx_close),
                Box::into_raw(ctx) as *mut _,
                XML_CHAR_ENCODING_NONE,
            );

            if parser.is_null() {
                // on error, xmlCreateIOParserCtxt() frees our ctx via the
                // stream_ctx_close function
                Err(LoadingError::CouldNotCreateXmlParser)
            } else {
                xml2_parser.parser.set(parser);

                set_xml_parse_options(parser, unlimited_size);

                Ok(xml2_parser)
            }
        }
    }

    pub fn parse(&self) -> Result<(), LoadingError> {
        unsafe {
            let parser = self.parser.get();

            let xml_parse_success = xmlParseDocument(parser) == 0;

            let mut err_ref = self.gio_error.borrow_mut();

            let io_error = err_ref.take();

            if let Some(io_error) = io_error {
                Err(LoadingError::Glib(io_error))
            } else if !xml_parse_success {
                let xerr = xmlCtxtGetLastError(parser as *mut _);
                let msg = xml2_error_to_string(xerr);
                Err(LoadingError::XmlParseError(msg))
            } else {
                Ok(())
            }
        }
    }
}

impl Drop for Xml2Parser {
    fn drop(&mut self) {
        let parser = self.parser.get();
        free_xml_parser_and_doc(parser);
        self.parser.set(ptr::null_mut());
    }
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
