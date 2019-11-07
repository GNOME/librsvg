use cssparser::{
    self,
    parse_important,
    AtRuleParser,
    CowRcStr,
    DeclarationParser,
    Parser,
    ParserInput,
    ToCss,
};
use selectors::attr::{AttrSelectorOperation, NamespaceConstraint, CaseSensitivity};
use selectors::matching::{ElementSelectorFlags, MatchingContext};
use selectors::{self, OpaqueElement, SelectorImpl};

use std::collections::hash_map::{Entry, Iter as HashMapIter};
use std::collections::HashMap;
use std::fmt;
use std::ptr;
use std::str;

use libc;
use markup5ever::{namespace_url, ns, LocalName, Namespace, Prefix, QualName};
use url::Url;

use glib::translate::*;
use glib_sys::{gboolean, gpointer, GList};

use crate::allowed_url::AllowedUrl;
use crate::croco::*;
use crate::error::*;
use crate::io::{self, BinaryData};
use crate::node::{NodeData, NodeType, RsvgNode};
use crate::properties::{parse_attribute_value_into_parsed_property, ParsedProperty};
use crate::text::NodeChars;
use crate::util::utf8_cstr;

/// A parsed CSS declaration (`name: value [!important]`)
pub struct Declaration {
    pub attribute: QualName,
    pub property: ParsedProperty,
    pub important: bool,
}

#[derive(Default)]
pub struct DeclarationList {
    // Maps property_name -> Declaration
    declarations: HashMap<QualName, Declaration>,
}

pub struct DeclParser;

impl<'i> DeclarationParser<'i> for DeclParser {
    type Declaration = Declaration;
    type Error = ValueErrorKind;

    fn parse_value<'t>(
        &mut self,
        name: CowRcStr<'i>,
        input: &mut Parser<'i, 't>,
    ) -> Result<Declaration, cssparser::ParseError<'i, ValueErrorKind>> {
        let attribute = QualName::new(None, ns!(svg), LocalName::from(name.as_ref()));
        let property = parse_attribute_value_into_parsed_property(&attribute, input, true)
            .map_err(|e| input.new_custom_error(e))?;

        let important = input.try_parse(parse_important).is_ok();

        Ok(Declaration {
            attribute,
            property,
            important,
        })
    }
}

impl<'i> AtRuleParser<'i> for DeclParser {
    type PreludeNoBlock = ();
    type PreludeBlock = ();
    type AtRule = Declaration;
    type Error = ValueErrorKind;
}

/// Dummy type required by the SelectorImpl trait
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct NonTSPseudoClass;

impl ToCss for NonTSPseudoClass {
    fn to_css<W>(&self, _dest: &mut W) -> fmt::Result where W: fmt::Write {
        Ok(())
    }
}

impl selectors::parser::NonTSPseudoClass for NonTSPseudoClass {
    type Impl = RsvgSelectors;

    fn is_active_or_hover(&self) -> bool {
        false
    }

    fn is_user_action_state(&self) -> bool {
        false
    }
}

/// Dummy type required by the SelectorImpl trait
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PseudoElement;

impl ToCss for PseudoElement {
    fn to_css<W>(&self, _dest: &mut W) -> fmt::Result where W: fmt::Write {
        Ok(())
    }
}

impl selectors::parser::PseudoElement for PseudoElement {
    type Impl = RsvgSelectors;
}

/// Holds all the types for the SelectorImpl trait
#[derive(Debug, Clone)]
pub struct RsvgSelectors;

impl SelectorImpl for RsvgSelectors {
    type ExtraMatchingData = ();
    type AttrValue = String;
    type Identifier = LocalName;
    type ClassName = LocalName;
    type PartName = LocalName;
    type LocalName = LocalName;
    type NamespaceUrl = Namespace;
    type NamespacePrefix = Prefix;
    type BorrowedNamespaceUrl = Namespace;
    type BorrowedLocalName = LocalName;
    type NonTSPseudoClass = NonTSPseudoClass;
    type PseudoElement = PseudoElement;
}

// We need a newtype because RsvgNode is an alias for rctree::Node
#[derive(Clone)]
pub struct RsvgElement(RsvgNode);

impl From<RsvgNode> for RsvgElement {
    fn from(n: RsvgNode) -> RsvgElement {
        RsvgElement(n)
    }
}

impl fmt::Debug for RsvgElement {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}", self.0.borrow())
    }
}

impl selectors::Element for RsvgElement {
    type Impl = RsvgSelectors;

    /// Converts self into an opaque representation.
    fn opaque(&self) -> OpaqueElement {
        OpaqueElement::new(&self.0.borrow())
    }

    fn parent_element(&self) -> Option<Self> {
        self.0.parent().map(|n| n.into())
    }

    /// Whether the parent node of this element is a shadow root.
    fn parent_node_is_shadow_root(&self) -> bool {
        // unsupported
        false
    }

    /// The host of the containing shadow root, if any.
    fn containing_shadow_host(&self) -> Option<Self> {
        // unsupported
        None
    }

    /// Whether we're matching on a pseudo-element.
    fn is_pseudo_element(&self) -> bool {
        // unsupported
        false
    }

    /// Skips non-element nodes
    fn prev_sibling_element(&self) -> Option<Self> {
        let mut sibling = self.0.previous_sibling();

        while let Some(ref sib) = sibling {
            if sib.borrow().get_type() != NodeType::Chars {
                return sibling.map(|n| n.into())
            }

            sibling = self.0.previous_sibling();
        }

        None
    }

    /// Skips non-element nodes
    fn next_sibling_element(&self) -> Option<Self> {
        let mut sibling = self.0.next_sibling();

        while let Some(ref sib) = sibling {
            if sib.borrow().get_type() != NodeType::Chars {
                return sibling.map(|n| n.into());
            }

            sibling = self.0.next_sibling();
        }

        None
    }

    fn is_html_element_in_html_document(&self) -> bool {
        false
    }

    fn has_local_name(&self, local_name: &LocalName) -> bool {
        self.0.borrow().element_name().local == *local_name
    }

    /// Empty string for no namespace
    fn has_namespace(&self, ns: &Namespace) -> bool {
        self.0.borrow().element_name().ns == *ns
    }

    /// Whether this element and the `other` element have the same local name and namespace.
    fn is_same_type(&self, other: &Self) -> bool {
        self.0.borrow().element_name() == other.0.borrow().element_name()
    }

    fn attr_matches(
        &self,
        _ns: &NamespaceConstraint<&Namespace>,
        _local_name: &LocalName,
        _operation: &AttrSelectorOperation<&String>,
    ) -> bool {
        // unsupported
        false
    }

    fn match_non_ts_pseudo_class<F>(
        &self,
        _pc: &<Self::Impl as SelectorImpl>::NonTSPseudoClass,
        _context: &mut MatchingContext<Self::Impl>,
        _flags_setter: &mut F,
    ) -> bool
    where
        F: FnMut(&Self, ElementSelectorFlags) {
        // unsupported
        false
    }

    fn match_pseudo_element(
        &self,
        _pe: &<Self::Impl as SelectorImpl>::PseudoElement,
        _context: &mut MatchingContext<Self::Impl>,
    ) -> bool {
        // unsupported
        false
    }

    /// Whether this element is a `link`.
    fn is_link(&self) -> bool {
        // FIXME: is this correct for SVG <a>, not HTML <a>?
        self.0.borrow().get_type() == NodeType::Link
    }

    /// Returns whether the element is an HTML <slot> element.
    fn is_html_slot_element(&self) -> bool {
        false
    }

    fn has_id(
        &self,
        id: &LocalName,
        case_sensitivity: CaseSensitivity,
    ) -> bool {
        self.0
            .borrow()
            .get_id()
            .map(|self_id| case_sensitivity.eq(self_id.as_bytes(), id.as_ref().as_bytes()))
            .unwrap_or(false)
    }

    fn has_class(
        &self,
        name: &LocalName,
        case_sensitivity: CaseSensitivity,
    ) -> bool {
        self.0
            .borrow()
            .get_class()
            .map(|classes| {
                classes
                    .split_whitespace()
                    .any(|class| case_sensitivity.eq(class.as_bytes(), name.as_bytes()))
            })
            .unwrap_or(false)
    }

    fn is_part(&self, _name: &LocalName) -> bool {
        // unsupported
        false
    }

    /// Returns whether this element matches `:empty`.
    ///
    /// That is, whether it does not contain any child element or any non-zero-length text node.
    /// See http://dev.w3.org/csswg/selectors-3/#empty-pseudo
    fn is_empty(&self) -> bool {
        !self.0.has_children() ||
            self.0.children().all(|child| {
                child.borrow().get_type() == NodeType::Chars
                    && child.borrow().get_impl::<NodeChars>().is_empty()
            })
    }

    /// Returns whether this element matches `:root`,
    /// i.e. whether it is the root element of a document.
    ///
    /// Note: this can be false even if `.parent_element()` is `None`
    /// if the parent node is a `DocumentFragment`.
    fn is_root(&self) -> bool {
        self.0.parent().is_none()
    }
}

#[derive(Clone, Hash, PartialEq, Eq)]
pub struct Selector {
    name: String,
    specificity: u64,
}

impl Selector {
    fn new(s: &str, specificity: u64) -> Selector {
        Selector {
            name: s.to_string(),
            specificity,
        }
    }
}

/// Contains all the mappings of selectors to style declarations
/// that result from loading an SVG document.
#[derive(Default)]
pub struct CssRules {
    selectors_to_declarations: HashMap<Selector, DeclarationList>,
}

impl DeclarationList {
    fn add_declaration(&mut self, declaration: Declaration) {
        match self.declarations.entry(declaration.attribute.clone()) {
            Entry::Occupied(mut e) => {
                let decl = e.get_mut();

                if !decl.important {
                    *decl = declaration;
                }
            }

            Entry::Vacant(v) => {
                v.insert(declaration);
            }
        }
    }

    pub fn iter(&self) -> DeclarationListIter {
        DeclarationListIter(self.declarations.iter())
    }
}

pub struct DeclarationListIter<'a>(HashMapIter<'a, QualName, Declaration>);

impl<'a> Iterator for DeclarationListIter<'a> {
    type Item = &'a Declaration;

    fn next(&mut self) -> Option<Self::Item> {
        self.0.next().map(|(_attribute, declaration)| declaration)
    }
}

impl CssRules {
    pub fn parse(&mut self, base_url: Option<&Url>, buf: &str) {
        if buf.is_empty() {
            return; // libcroco doesn't like empty strings :(
        }

        unsafe {
            let mut handler_data = DocHandlerData {
                base_url,
                css_rules: self,
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

    fn add_declaration(&mut self, selector: Selector, declaration: Declaration) {
        let decl_list = self
            .selectors_to_declarations
            .entry(selector)
            .or_insert_with(DeclarationList::default);

        decl_list.add_declaration(declaration);
    }

    pub fn get_declarations(&self, selector: &Selector) -> Option<&DeclarationList> {
        self.selectors_to_declarations.get(selector)
    }

    fn selector_matches_node(&self, selector: &Selector, node_data: &NodeData) -> bool {
        // Try to properly support all of the following, including inheritance:
        // *
        // #id
        // tag
        // tag#id
        // tag.class
        // tag.class#id
        //
        // This is basically a semi-compliant CSS2 selection engine

        let element_name = node_data.element_name().local.as_ref();
        let id = node_data.get_id();

        // *
        if selector.name == "*" {
            return true;
        }

        // tag
        if selector.name == element_name {
            return true;
        }

        if let Some(class) = node_data.get_class() {
            for cls in class.split_whitespace() {
                if !cls.is_empty() {
                    // tag.class#id
                    if let Some(id) = id {
                        let target = format!("{}.{}#{}", element_name, cls, id);
                        if selector.name == target {
                            return true;
                        }
                    }

                    // .class#id
                    if let Some(id) = id {
                        let target = format!(".{}#{}", cls, id);
                        if selector.name == target {
                            return true;
                        }
                    }

                    // tag.class
                    let target = format!("{}.{}", element_name, cls);
                    if selector.name == target {
                        return true;
                    }

                    // didn't find anything more specific, just apply the class style
                    let target = format!(".{}", cls);
                    if selector.name == target {
                        return true;
                    }
                }
            }
        }

        if let Some(id) = id {
            // id
            let target = format!("#{}", id);
            if selector.name == target {
                return true;
            }

            // tag#id
            let target = format!("{}#{}", element_name, id);
            if selector.name == target {
                return true;
            }
        }

        false
    }

    pub fn get_matches(&self, node_data: &NodeData) -> Vec<Selector> {
        let mut matches: Vec<_> = self.selectors_to_declarations
            .iter()
            .filter_map(|(selector, _)| {
                if self.selector_matches_node(selector, node_data) {
                    Some(selector)
                } else {
                    None
                }
            })
            .map(Selector::clone)
            .collect();

        matches.as_mut_slice().sort_by(|sel_a, sel_b| sel_a.specificity.cmp(&sel_b.specificity));

        matches
    }
}

struct DocHandlerData<'a> {
    base_url: Option<&'a Url>,
    css_rules: &'a mut CssRules,
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
        let _ = handler_data.css_rules.load_css(&aurl);
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

        cur_sel = (*cur_sel).next;

        if !simple_sel.is_null() {
            if cr_simple_sel_compute_specificity(simple_sel) != CR_OK {
                continue;
            }

            let specificity = u64::from((*simple_sel).specificity);

            let raw_selector_name = cr_simple_sel_to_string(simple_sel) as *mut libc::c_char;

            if !raw_selector_name.is_null() {
                let raw_prop_name = cr_string_peek_raw_str(a_name);
                let prop_name = utf8_cstr(raw_prop_name);

                let prop_value =
                    <String as FromGlibPtrFull<_>>::from_glib_full(cr_term_to_string(a_expression));

                let selector_name =
                    <String as FromGlibPtrFull<_>>::from_glib_full(raw_selector_name);

                let important = from_glib(a_is_important);

                let attribute = QualName::new(None, ns!(svg), LocalName::from(prop_name));

                let mut input = ParserInput::new(&prop_value);
                let mut parser = Parser::new(&mut input);

                match parse_attribute_value_into_parsed_property(&attribute, &mut parser, true) {
                    Ok(property) => {
                        let declaration = Declaration {
                            attribute,
                            property,
                            important,
                        };

                        handler_data
                            .css_rules
                            .add_declaration(Selector::new(&selector_name, specificity), declaration);
                    }
                    Err(_) => (), // invalid property name or invalid value; ignore
                }
            }
        }
    }
}

unsafe extern "C" fn css_error(_a_this: *mut CRDocHandler) {
    println!("CSS parsing error");
}

unsafe extern "C" fn css_unrecoverable_error(_a_this: *mut CRDocHandler) {
    println!("CSS unrecoverable error");
}
