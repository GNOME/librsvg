//! Representation of CSS types, and the CSS parsing and matching engine.
//!
//! # Terminology
//!
//! Consider a CSS **stylesheet** like this:
//!
//! ```ignore
//! @import url("another.css");
//!
//! foo, .bar {
//!         fill: red;
//!         stroke: green;
//! }
//!
//! #baz { stroke-width: 42; }
//! ```
//! The example contains three **rules**, the first one is an **at-rule*,
//! the other two are **qualified rules**.
//!
//! Each rule is made of two parts, a **prelude** and an optional **block**
//! The prelude is the part until the first `{` or until `;`, depending on
//! whether a block is present.  The block is the part between curly braces.
//!
//! Let's look at each rule:
//!
//! `@import` is an **at-rule**.  This rule has a prelude, but no block.
//! There are other at-rules like `@media` and some of them may have a block,
//! but librsvg doesn't support those yet.
//!
//! The prelude of the following rule is `foo, .bar`.
//! It is a **selector list** with two **selectors**, one for
//! `foo` elements and one for elements that have the `bar` class.
//!
//! The content of the block between `{}` for a qualified rule is a
//! **declaration list**.  The block of the first qualified rule contains two
//! **declarations**, one for the `fill` **property** and one for the
//! `stroke` property.
//!
//! After the first qualified rule, we have a second qualified rule with
//! a single selector for the `#baz` id, with a single declaration for the
//! `stroke-width` property.
//!
//! # Helper crates we use
//!
//! * `cssparser` crate as a CSS tokenizer, and some utilities to
//! parse CSS rules and declarations.
//!
//! * `selectors` crate for the representation of selectors and
//! selector lists, and for the matching engine.
//!
//! Both crates provide very generic implementations of their concepts,
//! and expect the caller to provide implementations of various traits,
//! and to provide types that represent certain things.
//!
//! For example, `cssparser` expects one to provide representations of
//! the following types:
//!
//! * A parsed CSS rule.  For `fill: blue;` we have
//! `ParsedProperty::Fill(...)`.
//!
//! * A parsed selector list; we use `SelectorList` from the
//! `selectors` crate.
//!
//! In turn, the `selectors` crate needs a way to navigate and examine
//! one's implementation of an element tree.  We provide `impl
//! selectors::Element for RsvgElement` for this.  This implementation
//! has methods like "does this element have the id `#foo`", or "give
//! me the next sibling element".
//!
//! Finally, the matching engine ties all of this together with
//! `matches_selector()`.  This takes an opaque representation of an
//! element, plus a selector, and returns a bool.  We iterate through
//! the rules in the stylesheets and gather the matches; then sort the
//! matches by specificity and apply the result to each element.

use cssparser::{
    self, match_ignore_ascii_case, parse_important, AtRuleParser, AtRuleType, BasicParseErrorKind,
    CowRcStr, DeclarationListParser, DeclarationParser, Parser, ParserInput, QualifiedRuleParser,
    RuleListParser, SourceLocation, ToCss, _cssparser_internal_to_lowercase,
};
use markup5ever::{namespace_url, ns, LocalName, Namespace, Prefix, QualName};
use selectors::attr::{AttrSelectorOperation, CaseSensitivity, NamespaceConstraint};
use selectors::matching::{ElementSelectorFlags, MatchingContext, MatchingMode, QuirksMode};
use selectors::{OpaqueElement, SelectorImpl, SelectorList};
use std::cmp::Ordering;
use std::fmt;
use std::str;
use url::Url;

use crate::allowed_url::AllowedUrl;
use crate::error::*;
use crate::io::{self, BinaryData};
use crate::node::{Node, NodeBorrow, NodeCascade};
use crate::properties::{parse_property, ComputedValues, ParsedProperty};

/// A parsed CSS declaration
///
/// For example, in the declaration `fill: green !important`, the
/// `prop_name` would be `fill`, the `property` would be
/// `ParsedProperty::Fill(...)` with the green value, and `important`
/// would be `true`.
pub struct Declaration {
    pub prop_name: QualName,
    pub property: ParsedProperty,
    pub important: bool,
}

/// Dummy struct required to use `cssparser::DeclarationListParser`
///
/// It implements `cssparser::DeclarationParser`, which knows how to parse
/// the property/value pairs from a CSS declaration.
pub struct DeclParser;

impl<'i> DeclarationParser<'i> for DeclParser {
    type Declaration = Declaration;
    type Error = ValueErrorKind;

    /// Parses a CSS declaration like `name: input_value [!important]`
    fn parse_value<'t>(
        &mut self,
        name: CowRcStr<'i>,
        input: &mut Parser<'i, 't>,
    ) -> Result<Declaration, ParseError<'i>> {
        let prop_name = QualName::new(None, ns!(), LocalName::from(name.as_ref()));
        let property = parse_property(&prop_name, input, true)?;

        let important = input.try_parse(parse_important).is_ok();

        Ok(Declaration {
            prop_name,
            property,
            important,
        })
    }
}

// cssparser's DeclarationListParser requires this; we just use the dummy
// implementations from cssparser itself.  We may want to provide a real
// implementation in the future, although this may require keeping track of the
// CSS parsing state like Servo does.
impl<'i> AtRuleParser<'i> for DeclParser {
    type PreludeBlock = ();
    type PreludeNoBlock = ();
    type AtRule = Declaration;
    type Error = ValueErrorKind;
}

/// Dummy struct to implement cssparser::QualifiedRuleParser and
/// cssparser::AtRuleParser
pub struct RuleParser;

/// Errors from the CSS parsing process
#[derive(Debug)]
pub enum ParseErrorKind<'i> {
    Selector(selectors::parser::SelectorParseErrorKind<'i>),
}

impl<'i> From<selectors::parser::SelectorParseErrorKind<'i>> for ParseErrorKind<'i> {
    fn from(e: selectors::parser::SelectorParseErrorKind) -> ParseErrorKind {
        ParseErrorKind::Selector(e)
    }
}

/// A CSS qualified rule (or ruleset)
pub struct QualifiedRule {
    selectors: SelectorList<Selector>,
    declarations: Vec<Declaration>,
}

/// Prelude of at-rule used in the AtRuleParser.
pub enum AtRulePrelude {
    Import(String),
}

/// A CSS at-rule (or ruleset)
pub enum AtRule {
    Import(String),
}

/// A CSS rule (or ruleset)
pub enum Rule {
    AtRule(AtRule),
    QualifiedRule(QualifiedRule),
}

// Required to implement the `Prelude` associated type in `cssparser::QualifiedRuleParser`
impl<'i> selectors::Parser<'i> for RuleParser {
    type Impl = Selector;
    type Error = ParseErrorKind<'i>;

    fn default_namespace(&self) -> Option<<Self::Impl as SelectorImpl>::NamespaceUrl> {
        Some(ns!(svg))
    }

    fn namespace_for_prefix(
        &self,
        _prefix: &<Self::Impl as SelectorImpl>::NamespacePrefix,
    ) -> Option<<Self::Impl as SelectorImpl>::NamespaceUrl> {
        // FIXME: Do we need to keep a lookup table extracted from libxml2's
        // XML namespaces?
        //
        // Or are CSS namespaces completely different, declared elsewhere?
        None
    }
}

// `cssparser::RuleListParser` is a struct which requires that we
// provide a type that implements `cssparser::QualifiedRuleParser`.
//
// In turn, `cssparser::QualifiedRuleParser` requires that we
// implement a way to parse the `Prelude` of a ruleset or rule.  For
// example, in this ruleset:
//
// ```ignore
// foo, .bar { fill: red; stroke: green; }
// ```
//
// The prelude is the selector list with the `foo` and `.bar` selectors.
//
// The `parse_prelude` method just uses `selectors::SelectorList`.  This
// is what requires the `impl selectors::Parser for RuleParser`.
//
// Next, the `parse_block` method takes an already-parsed prelude (a selector list),
// and tries to parse the block between braces.  It creates a `Rule` out of
// the selector list and the declaration list.
impl<'i> QualifiedRuleParser<'i> for RuleParser {
    type Prelude = SelectorList<Selector>;
    type QualifiedRule = Rule;
    type Error = ParseErrorKind<'i>;

    fn parse_prelude<'t>(
        &mut self,
        input: &mut Parser<'i, 't>,
    ) -> Result<Self::Prelude, cssparser::ParseError<'i, Self::Error>> {
        SelectorList::parse(self, input)
    }

    fn parse_block<'t>(
        &mut self,
        prelude: Self::Prelude,
        _location: SourceLocation,
        input: &mut Parser<'i, 't>,
    ) -> Result<Self::QualifiedRule, cssparser::ParseError<'i, Self::Error>> {
        let declarations = DeclarationListParser::new(input, DeclParser)
            .filter_map(|r| match r {
                Ok(decl) => Some(decl),
                Err(e) => {
                    rsvg_log!("Invalid declaration; ignoring: {:?}", e);
                    None
                }
            })
            .collect();

        Ok(Rule::QualifiedRule(QualifiedRule {
            selectors: prelude,
            declarations,
        }))
    }
}

// Required by `cssparser::RuleListParser`.
//
// This only handles the `@import` at-rule.
impl<'i> AtRuleParser<'i> for RuleParser {
    type PreludeBlock = ();
    type PreludeNoBlock = AtRulePrelude;
    type AtRule = Rule;
    type Error = ParseErrorKind<'i>;

    #[allow(clippy::type_complexity)]
    fn parse_prelude<'t>(
        &mut self,
        name: CowRcStr<'i>,
        input: &mut Parser<'i, 't>,
    ) -> Result<
        AtRuleType<Self::PreludeNoBlock, Self::PreludeBlock>,
        cssparser::ParseError<'i, Self::Error>,
    > {
        match_ignore_ascii_case! { &name,
            "import" => {
                // FIXME: at the moment we ignore media queries
                let url = input.expect_url_or_string()?.as_ref().to_owned();
                Ok(AtRuleType::WithoutBlock(AtRulePrelude::Import(url)))
            },

            _ => Err(input.new_error(BasicParseErrorKind::AtRuleInvalid(name))),
        }
    }

    fn rule_without_block(
        &mut self,
        prelude: Self::PreludeNoBlock,
        _location: SourceLocation,
    ) -> Self::AtRule {
        let AtRulePrelude::Import(url) = prelude;
        Rule::AtRule(AtRule::Import(url))
    }
}

/// Dummy type required by the SelectorImpl trait.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct NonTSPseudoClass;

impl ToCss for NonTSPseudoClass {
    fn to_css<W>(&self, _dest: &mut W) -> fmt::Result
    where
        W: fmt::Write,
    {
        Ok(())
    }
}

impl selectors::parser::NonTSPseudoClass for NonTSPseudoClass {
    type Impl = Selector;

    fn is_active_or_hover(&self) -> bool {
        false
    }

    fn is_user_action_state(&self) -> bool {
        false
    }

    fn has_zero_specificity(&self) -> bool {
        false
    }
}

/// Dummy type required by the SelectorImpl trait
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PseudoElement;

impl ToCss for PseudoElement {
    fn to_css<W>(&self, _dest: &mut W) -> fmt::Result
    where
        W: fmt::Write,
    {
        Ok(())
    }
}

impl selectors::parser::PseudoElement for PseudoElement {
    type Impl = Selector;
}

/// Holds all the types for the SelectorImpl trait
#[derive(Debug, Clone)]
pub struct Selector;

impl SelectorImpl for Selector {
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

/// Wraps an `Node` with a locally-defined type, so we can implement
/// a foreign trait on it.
///
/// `Node` is an alias for `rctree::Node`, so we can't implement
/// `selectors::Element` directly on it.  We implement it on the
/// `RsvgElement` wrapper instead.
#[derive(Clone, PartialEq)]
pub struct RsvgElement(Node);

impl From<Node> for RsvgElement {
    fn from(n: Node) -> RsvgElement {
        RsvgElement(n)
    }
}

impl fmt::Debug for RsvgElement {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}", self.0.borrow())
    }
}

// The selectors crate uses this to examine our tree of elements.
impl selectors::Element for RsvgElement {
    type Impl = Selector;

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
            if sib.is_element() {
                return sibling.map(|n| n.into());
            }

            sibling = sib.previous_sibling();
        }

        None
    }

    /// Skips non-element nodes
    fn next_sibling_element(&self) -> Option<Self> {
        let mut sibling = self.0.next_sibling();

        while let Some(ref sib) = sibling {
            if sib.is_element() {
                return sibling.map(|n| n.into());
            }

            sibling = sib.next_sibling();
        }

        None
    }

    fn is_html_element_in_html_document(&self) -> bool {
        false
    }

    fn has_local_name(&self, local_name: &LocalName) -> bool {
        self.0.borrow_element().element_name().local == *local_name
    }

    /// Empty string for no namespace
    fn has_namespace(&self, ns: &Namespace) -> bool {
        self.0.borrow_element().element_name().ns == *ns
    }

    /// Whether this element and the `other` element have the same local name and namespace.
    fn is_same_type(&self, other: &Self) -> bool {
        self.0.borrow_element().element_name() == other.0.borrow_element().element_name()
    }

    fn attr_matches(
        &self,
        ns: &NamespaceConstraint<&Namespace>,
        local_name: &LocalName,
        operation: &AttrSelectorOperation<&String>,
    ) -> bool {
        self.0
            .borrow_element()
            .get_attributes()
            .iter()
            .find(|(attr, _)| {
                // do we have an attribute that matches the namespace and local_name?
                match *ns {
                    NamespaceConstraint::Any => *local_name == attr.local,
                    NamespaceConstraint::Specific(ns) => {
                        QualName::new(None, ns.clone(), local_name.clone()) == *attr
                    }
                }
            })
            .map(|(_, value)| {
                // we have one; does the attribute's value match the expected operation?
                operation.eval_str(value)
            })
            .unwrap_or(false)
    }

    fn match_non_ts_pseudo_class<F>(
        &self,
        _pc: &<Self::Impl as SelectorImpl>::NonTSPseudoClass,
        _context: &mut MatchingContext<Self::Impl>,
        _flags_setter: &mut F,
    ) -> bool
    where
        F: FnMut(&Self, ElementSelectorFlags),
    {
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
        self.0.is_element() && is_element_of_type!(self.0, Link)
    }

    /// Returns whether the element is an HTML <slot> element.
    fn is_html_slot_element(&self) -> bool {
        false
    }

    fn has_id(&self, id: &LocalName, case_sensitivity: CaseSensitivity) -> bool {
        self.0
            .borrow_element()
            .get_id()
            .map(|self_id| case_sensitivity.eq(self_id.as_bytes(), id.as_ref().as_bytes()))
            .unwrap_or(false)
    }

    fn has_class(&self, name: &LocalName, case_sensitivity: CaseSensitivity) -> bool {
        self.0
            .borrow_element()
            .get_class()
            .map(|classes| {
                classes
                    .split_whitespace()
                    .any(|class| case_sensitivity.eq(class.as_bytes(), name.as_bytes()))
            })
            .unwrap_or(false)
    }

    fn exported_part(&self, _name: &LocalName) -> Option<LocalName> {
        // unsupported
        None
    }

    fn imported_part(&self, _name: &LocalName) -> Option<LocalName> {
        // unsupported
        None
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
        // .all() returns true for the empty iterator
        self.0
            .children()
            .all(|child| child.is_chars() && child.borrow_chars().is_empty())
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

/// Origin for a stylesheet, per https://www.w3.org/TR/CSS22/cascade.html#cascading-order
///
/// This is used when sorting selector matches according to their origin and specificity.
#[derive(Copy, Clone, Eq, Ord, PartialEq, PartialOrd)]
pub enum Origin {
    UserAgent,
    User,
    Author,
}

/// A parsed CSS stylesheet
pub struct Stylesheet {
    origin: Origin,
    qualified_rules: Vec<QualifiedRule>,
}

/// A match during the selector matching process
///
/// This struct comes from `Stylesheet.get_matches()`, and represents
/// that a certain node matched a CSS rule which has a selector with a
/// certain `specificity`.  The stylesheet's `origin` is also given here.
///
/// This type implements `Ord` so a list of `Match` can be sorted.
/// That implementation does ordering based on origin and specificity
/// as per https://www.w3.org/TR/CSS22/cascade.html#cascading-order
struct Match<'a> {
    specificity: u32,
    origin: Origin,
    declaration: &'a Declaration,
}

impl<'a> Ord for Match<'a> {
    fn cmp(&self, other: &Self) -> Ordering {
        match self.origin.cmp(&other.origin) {
            Ordering::Equal => self.specificity.cmp(&other.specificity),
            o => o,
        }
    }
}

impl<'a> PartialOrd for Match<'a> {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl<'a> PartialEq for Match<'a> {
    fn eq(&self, other: &Self) -> bool {
        self.origin == other.origin && self.specificity == other.specificity
    }
}

impl<'a> Eq for Match<'a> {}

impl Stylesheet {
    pub fn new(origin: Origin) -> Stylesheet {
        Stylesheet {
            origin,
            qualified_rules: Vec::new(),
        }
    }

    pub fn from_data(
        buf: &str,
        base_url: Option<&Url>,
        origin: Origin,
    ) -> Result<Self, LoadingError> {
        let mut stylesheet = Stylesheet::new(origin);
        stylesheet.parse(buf, base_url)?;
        Ok(stylesheet)
    }

    pub fn from_href(
        href: &str,
        base_url: Option<&Url>,
        origin: Origin,
    ) -> Result<Self, LoadingError> {
        let mut stylesheet = Stylesheet::new(origin);
        stylesheet.load(href, base_url)?;
        Ok(stylesheet)
    }

    /// Parses a CSS stylesheet from a string
    ///
    /// The `base_url` is required for `@import` rules, so that librsvg
    /// can determine if the requested path is allowed.
    pub fn parse(&mut self, buf: &str, base_url: Option<&Url>) -> Result<(), LoadingError> {
        let mut input = ParserInput::new(buf);
        let mut parser = Parser::new(&mut input);

        RuleListParser::new_for_stylesheet(&mut parser, RuleParser)
            .filter_map(|r| match r {
                Ok(rule) => Some(rule),
                Err(e) => {
                    rsvg_log!("Invalid rule; ignoring: {:?}", e);
                    None
                }
            })
            .for_each(|rule| match rule {
                Rule::AtRule(AtRule::Import(url)) => {
                    // ignore invalid imports
                    let _ = self.load(&url, base_url);
                }
                Rule::QualifiedRule(qr) => self.qualified_rules.push(qr),
            });

        Ok(())
    }

    /// Parses a stylesheet referenced by an URL
    fn load(&mut self, href: &str, base_url: Option<&Url>) -> Result<(), LoadingError> {
        let aurl = AllowedUrl::from_href(href, base_url).map_err(|_| LoadingError::BadUrl)?;

        io::acquire_data(&aurl, None)
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
            .and_then(|utf8| self.parse(&utf8, Some(&aurl)))
    }

    /// Appends the style declarations that match a specified node to a given vector
    fn get_matches<'a>(
        &'a self,
        node: &Node,
        match_ctx: &mut MatchingContext<Selector>,
        acc: &mut Vec<Match<'a>>,
    ) {
        for rule in &self.qualified_rules {
            for selector in &rule.selectors.0 {
                // This magic call is stolen from selectors::matching::matches_selector_list()
                let matches = selectors::matching::matches_selector(
                    selector,
                    0,
                    None,
                    &RsvgElement(node.clone()),
                    match_ctx,
                    &mut |_, _| {},
                );

                if matches {
                    for decl in rule.declarations.iter() {
                        acc.push(Match {
                            declaration: decl,
                            specificity: selector.specificity(),
                            origin: self.origin,
                        });
                    }
                }
            }
        }
    }
}

/// Runs the CSS cascade on the specified tree from all the stylesheets
pub fn cascade(
    root: &mut Node,
    ua_stylesheets: &[Stylesheet],
    author_stylesheets: &[Stylesheet],
    user_stylesheets: &[Stylesheet],
) {
    for mut node in root.descendants().filter(|n| n.is_element()) {
        let mut matches = Vec::new();

        let mut match_ctx = MatchingContext::new(
            MatchingMode::Normal,
            // FIXME: how the fuck does one set up a bloom filter here?
            None,
            // n_index_cache,
            None,
            QuirksMode::NoQuirks,
        );

        for s in ua_stylesheets
            .iter()
            .chain(author_stylesheets)
            .chain(user_stylesheets)
        {
            s.get_matches(&node, &mut match_ctx, &mut matches);
        }

        matches.as_mut_slice().sort();

        for m in matches {
            node.borrow_element_mut()
                .apply_style_declaration(m.declaration, m.origin);
        }

        node.borrow_element_mut().set_style_attribute();
    }

    let values = ComputedValues::default();
    root.cascade(&values);
}

#[cfg(test)]
mod tests {
    use super::*;
    use gio;
    use glib::{self, prelude::*};
    use selectors::Element;

    use crate::allowed_url::Fragment;
    use crate::document::Document;
    use crate::handle::LoadOptions;

    fn load_document(input: &'static [u8]) -> Document {
        let bytes = glib::Bytes::from_static(input);
        let stream = gio::MemoryInputStream::new_from_bytes(&bytes);

        Document::load_from_stream(
            &LoadOptions::new(None),
            &stream.upcast(),
            None::<&gio::Cancellable>,
        )
        .unwrap()
    }

    #[test]
    fn impl_element() {
        let document = load_document(
            br#"<?xml version="1.0" encoding="UTF-8"?>
<svg xmlns="http://www.w3.org/2000/svg" id="a">
  <rect id="b" x="10" y="10" width="30" height="30"/>
  <circle id="c" cx="10" cy="10" r="10"/>
  <rect id="d" class="foo bar"/>
</svg>
"#,
        );

        let a = document
            .lookup(&Fragment::new(None, "a".to_string()))
            .unwrap();
        let b = document
            .lookup(&Fragment::new(None, "b".to_string()))
            .unwrap();
        let c = document
            .lookup(&Fragment::new(None, "c".to_string()))
            .unwrap();
        let d = document
            .lookup(&Fragment::new(None, "d".to_string()))
            .unwrap();

        // Node types
        assert!(is_element_of_type!(a, Svg));
        assert!(is_element_of_type!(b, Rect));
        assert!(is_element_of_type!(c, Circle));
        assert!(is_element_of_type!(d, Rect));

        let a = RsvgElement(a);
        let b = RsvgElement(b);
        let c = RsvgElement(c);
        let d = RsvgElement(d);

        // Tree navigation

        assert_eq!(a.parent_element(), None);
        assert_eq!(b.parent_element(), Some(a.clone()));
        assert_eq!(c.parent_element(), Some(a.clone()));
        assert_eq!(d.parent_element(), Some(a.clone()));

        assert_eq!(b.next_sibling_element(), Some(c.clone()));
        assert_eq!(c.next_sibling_element(), Some(d.clone()));
        assert_eq!(d.next_sibling_element(), None);

        assert_eq!(b.prev_sibling_element(), None);
        assert_eq!(c.prev_sibling_element(), Some(b.clone()));
        assert_eq!(d.prev_sibling_element(), Some(c.clone()));

        // Other operations

        assert!(a.has_local_name(&LocalName::from("svg")));

        assert!(a.has_namespace(&ns!(svg)));

        assert!(!a.is_same_type(&b));
        assert!(b.is_same_type(&d));

        assert!(a.has_id(&LocalName::from("a"), CaseSensitivity::AsciiCaseInsensitive));
        assert!(!b.has_id(
            &LocalName::from("foo"),
            CaseSensitivity::AsciiCaseInsensitive
        ));

        assert!(d.has_class(
            &LocalName::from("foo"),
            CaseSensitivity::AsciiCaseInsensitive
        ));
        assert!(d.has_class(
            &LocalName::from("bar"),
            CaseSensitivity::AsciiCaseInsensitive
        ));

        assert!(!a.has_class(
            &LocalName::from("foo"),
            CaseSensitivity::AsciiCaseInsensitive
        ));

        assert!(d.is_empty());
        assert!(!a.is_empty());
    }
}
