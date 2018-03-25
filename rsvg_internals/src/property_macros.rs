/// Generates a property definition that simply parses strings to enum variants
///
/// This can be used for properties with simple symbol-based values.
/// For example, the SVG spec defines the `stroke-linejoin` property
/// to have possible values `miter | round | bevel | inherit`, with a default
/// of `miter`.  We can define the property like this:
///
/// ```
/// make_ident_property!(
/// StrokeLinejoin,
/// default: Miter,
///
/// "miter" => Miter,
/// "round" => Round,
/// "bevel" => Bevel,
/// "inherit" => Inherit,
/// );
/// ```
///
/// The macro will generate a `StrokeLinejoin` enum with the provided
/// variants.  It will generate an `impl Default for StrokeLinejoin`
/// with the provided `default:` value.  Finally, it will generate an
/// `impl Parse for StrokeLinejoin`, from `parsers::Parse`, where
/// `type Data = ()` and `type Err = AttributeError`.
#[macro_export]
macro_rules! make_ident_property {
    ($name: ident,
     default: $default: ident,
     $($str_prop: expr => $variant: ident,)+
    ) => {
        #[repr(C)]
        #[derive(Debug, Copy, Clone)]
        pub enum $name {
            $($variant),+
        }

        impl Default for $name {
            fn default() -> $name {
                $name::$default
            }
        }

        impl Parse for $name {
            type Data = ();
            type Err = AttributeError;

            fn parse(s: &str, _: Self::Data) -> Result<$name, AttributeError> {
                match s.trim() {
                    $($str_prop => Ok($name::$variant),)+

                    _ => Err(AttributeError::from(ParseError::new("invalid value"))),
                }
            }
        }
    };
}
