//! Handling of `xlink:href` and `href` attributes
//!
//! In SVG1.1, links to elements are done with the `xlink:href` attribute.  However, SVG2
//! reduced this to just plain `href` with no namespace:
//! <https://svgwg.org/svg2-draft/linking.html#XLinkRefAttrs>
//!
//! If an element has both `xlink:href` and `href` attributes, the `href` overrides the
//! other.  We implement that logic in this module.

use markup5ever::{expanded_name, local_name, namespace_url, ns, ExpandedName};

/// Returns whether the attribute is either of `xlink:href` or `href`.
///
/// # Example
///
/// Use with an `if` pattern inside a `match`:
///
/// ```
/// # use markup5ever::{expanded_name, local_name, namespace_url, ns, QualName, Prefix, Namespace, LocalName, ExpandedName};
/// # use librsvg::doctest_only::{is_href,set_href};
///
/// let qual_name = QualName::new(
///     Some(Prefix::from("xlink")),
///     Namespace::from("http://www.w3.org/1999/xlink"),
///     LocalName::from("href"),
/// );
///
/// let value = expanded_name!("", "path");
/// let mut foo = Some(value);
///
/// match qual_name.expanded() {
///     ref name if is_href(name) => set_href(name, &mut foo, value),
///     _ => unreachable!(),
/// }
/// ```
pub fn is_href(name: &ExpandedName<'_>) -> bool {
    matches!(
        *name,
        expanded_name!(xlink "href") | expanded_name!("", "href")
    )
}

/// Sets an `href` attribute in preference over an `xlink:href` one.
///
/// See [`is_href`] for example usage.
pub fn set_href<T>(name: &ExpandedName<'_>, dest: &mut Option<T>, href: T) {
    if dest.is_none() || *name != expanded_name!(xlink "href") {
        *dest = Some(href);
    }
}
