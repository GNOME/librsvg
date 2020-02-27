/// This is a mitigation for the security-related bug
/// https://gitlab.gnome.org/GNOME/librsvg/issues/323 - imagine
/// the XML [billion laughs attack], but done by creating deeply
/// nested groups of `<use>` elements.  The first one references
/// the second one ten times, the second one references the third
/// one ten times, and so on.  In the file given, this causes
/// 10^17 objects to be rendered.  While this does not exhaust
/// memory, it would take a really long time.
///
/// [billion laughs attack]: https://bitbucket.org/tiran/defusedxml
pub const MAX_REFERENCED_ELEMENTS: usize = 500_000;
