use cssparser::{
    self,
    parse_important,
    AtRuleParser,
    CowRcStr,
    DeclarationListParser,
    DeclarationParser,
    Parser,
    ParserInput,
    QualifiedRuleParser,
    RuleListParser,
    SourceLocation,
    ToCss,
};
use selectors::attr::{AttrSelectorOperation, NamespaceConstraint, CaseSensitivity};
use selectors::matching::{ElementSelectorFlags, MatchingContext, MatchingMode, QuirksMode};
use selectors::{self, OpaqueElement, SelectorImpl, SelectorList};

use std::collections::hash_map::{Iter as HashMapIter};
use std::collections::HashMap;
use std::fmt;
use std::str;

use markup5ever::{namespace_url, ns, LocalName, Namespace, Prefix, QualName};
use url::Url;

use crate::allowed_url::AllowedUrl;
use crate::error::*;
use crate::io::{self, BinaryData};
use crate::node::{NodeType, RsvgNode};
use crate::properties::{parse_attribute_value_into_parsed_property, ParsedProperty};
use crate::text::NodeChars;

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

/// Dummy struct to implement cssparser::QualifiedRuleParser
pub struct QualRuleParser;

pub enum CssParseErrorKind<'i> {
    Selector(selectors::parser::SelectorParseErrorKind<'i>),
    Value(ValueErrorKind),
}

impl<'i> From<selectors::parser::SelectorParseErrorKind<'i>> for CssParseErrorKind<'i> {
    fn from(e: selectors::parser::SelectorParseErrorKind) -> CssParseErrorKind {
        CssParseErrorKind::Selector(e)
    }
}

/// A CSS ruleset (or rule)
pub struct Rule {
    selectors: SelectorList<RsvgSelectors>,
    declarations: DeclarationList,
}

impl<'i> selectors::Parser<'i> for QualRuleParser {
    type Impl = RsvgSelectors;
    type Error = CssParseErrorKind<'i>;

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

impl<'i> QualifiedRuleParser<'i> for QualRuleParser {
    type Prelude = SelectorList<RsvgSelectors>;
    type QualifiedRule = Rule;
    type Error = CssParseErrorKind<'i>;

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
        let decl_parser = DeclarationListParser::new(input, DeclParser);

        let mut decl_list = DeclarationList {
            declarations: HashMap::new(),
        };

        for decl_result in decl_parser {
            // ignore invalid property name or value
            if let Ok(declaration) = decl_result {
                decl_list.declarations.insert(declaration.attribute.clone(), declaration);
            }
        }

        Ok(Rule {
            selectors: prelude,
            declarations: decl_list,
        })
    }
}

impl<'i> AtRuleParser<'i> for QualRuleParser {
    type PreludeNoBlock = ();
    type PreludeBlock = ();
    type AtRule = Rule;
    type Error = CssParseErrorKind<'i>;
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

/// A parsed CSS stylesheet
#[derive(Default)]
pub struct Stylesheet {
    rules: Vec<Rule>,
}

impl DeclarationList {
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

impl Stylesheet {
    pub fn parse(&mut self, base_url: Option<&Url>, buf: &str) {
        let mut input = ParserInput::new(buf);
        let mut parser = Parser::new(&mut input);

        let rule_parser = RuleListParser::new_for_stylesheet(&mut parser, QualRuleParser);

        for rule_result in rule_parser {
            // Ignore invalid rules
            if let Ok(rule) = rule_result {
                self.rules.push(rule);
            }
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

    pub fn apply_matches_to_node(&self, node: &mut RsvgNode) {
        let mut match_ctx = MatchingContext::new(
            MatchingMode::Normal,

            // FIXME: how the fuck does one set up a bloom filter here?
            None,

            // n_index_cache,
            None,

            QuirksMode::NoQuirks,
        );

        for rule in &self.rules {
            if selectors::matching::matches_selector_list(
                &rule.selectors,
                &RsvgElement(node.clone()),
                &mut match_ctx,
            ) {
                for decl in rule.declarations.iter() {
                    node.borrow_mut().apply_style_declaration(decl);
                }
            }
        }
    }
}
