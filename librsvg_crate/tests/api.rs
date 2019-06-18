use librsvg::{DefsLookupErrorKind, HrefError, RenderingError};

mod utils;
use self::utils::load_svg;

#[test]
fn has_element_with_id_works() {
    let svg = load_svg(
        br#"<?xml version="1.0" encoding="UTF-8"?>
<svg xmlns="http://www.w3.org/2000/svg" width="50" height="50">
  <rect id="foo" x="10" y="10" width="30" height="30"/>
</svg>
"#,
    );

    assert!(svg.has_element_with_id("#foo").unwrap());
    assert!(!svg.has_element_with_id("#bar").unwrap());

    assert_eq!(
        svg.has_element_with_id(""),
        Err(RenderingError::InvalidId(DefsLookupErrorKind::HrefError(
            HrefError::ParseError
        )))
    );

    assert_eq!(
        svg.has_element_with_id("not a fragment"),
        Err(RenderingError::InvalidId(
            DefsLookupErrorKind::CannotLookupExternalReferences
        ))
    );

    assert_eq!(
        svg.has_element_with_id("notfragment#fragment"),
        Err(RenderingError::InvalidId(
            DefsLookupErrorKind::CannotLookupExternalReferences
        ))
    );
}
