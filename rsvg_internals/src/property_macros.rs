pub trait Property {
    fn inherits_automatically() -> bool;
}

/// Generates a property definition that simply parses strings to enum variants
/// or to a tuple struct of the given type.
///
/// For example, the SVG spec defines the `stroke-linejoin` property
/// to have possible values `miter | round | bevel | inherit`, with a default
/// of `miter`.  We can define the property like this:
///
/// ```
/// make_property!(
/// StrokeLinejoin,
/// default: Miter,
///
/// "miter" => Miter,
/// "round" => Round,
/// "bevel" => Bevel,
/// );
/// ```
///
/// The macro will generate a `StrokeLinejoin` enum with the provided
/// variants.  It will generate an `impl Default for StrokeLinejoin`
/// with the provided `default:` value.  Finally, it will generate an
/// `impl Parse for StrokeLinejoin`, from `parsers::Parse`, where
/// `type Data = ()` and `type Err = AttributeError`.
#[macro_export]
macro_rules! make_property {
    ($name: ident,
     default: $default: ident,
     inherits_automatically: $inherits_automatically: expr,
     $($str_prop: expr => $variant: ident,)+
    ) => {
        #[derive(Debug, Copy, Clone, PartialEq)]
        pub enum $name {
            $($variant),+
        }

        impl Default for $name {
            fn default() -> $name {
                $name::$default
            }
        }

        impl ::property_macros::Property for $name {
            fn inherits_automatically() -> bool {
                $inherits_automatically
            }
        }

        impl ::parsers::Parse for $name {
            type Data = ();
            type Err = ::error::AttributeError;

            fn parse(s: &str, _: Self::Data) -> Result<$name, ::error::AttributeError> {
                match s.trim() {
                    $($str_prop => Ok($name::$variant),)+

                    _ => Err(::error::AttributeError::from(::parsers::ParseError::new("invalid value"))),
                }
            }
        }
    };

    ($name: ident,
     default: $default: expr,
     inherits_automatically: $inherits_automatically: expr,
     $type: ty
    ) => {
        #[derive(Debug, Clone, PartialEq)]
        pub struct $name(pub $type);

        impl Default for $name {
            fn default() -> $name {
                $name($default)
            }
        }

        impl ::property_macros::Property for $name {
            fn inherits_automatically() -> bool {
                $inherits_automatically
            }
        }

        impl ::parsers::Parse for $name {
            type Data = ();
            type Err = ::error::AttributeError;

            fn parse(s: &str, _: Self::Data) -> Result<$name, ::error::AttributeError> {
                match s.trim().parse() {
                    Ok(val) => Ok($name(val)),

                    // FIXME: should this convert the string::ParseError into AttributeError?
                    _ => Err(::error::AttributeError::from(::parsers::ParseError::new("invalid value"))),
                }
            }
        }
    };
}

#[cfg(test)]
mod tests {
    use super::*;

    use parsers::Parse;

    #[test]
    fn check_generated_property() {
        make_property! {
            Foo,
            default: Def,
            inherits_automatically: true,

            "def" => Def,
            "bar" => Bar,
            "baz" => Baz,
        }

        assert_eq!(<Foo as Default>::default(), Foo::Def);
        assert_eq!(<Foo as Property>::inherits_automatically(), true);
        assert!(<Foo as Parse>::parse("blargh", ()).is_err());
        assert_eq!(<Foo as Parse>::parse("bar", ()), Ok(Foo::Bar));

        make_property! {
            Bar,
            default: "bar".to_string(),
            inherits_automatically: true,
            String
        }

        assert_eq!(<Bar as Default>::default(), Bar("bar".to_string()));
        assert_eq!(<Bar as Property>::inherits_automatically(), true);
        assert_eq!(<Bar as Parse>::parse("test", ()), Ok(Bar("test".to_string())));

        make_property! {
            Baz,
            default: 42f64,
            inherits_automatically: true,
            f64
        }

        assert_eq!(<Baz as Default>::default(), Baz(42f64));
        assert_eq!(<Baz as Property>::inherits_automatically(), true);
        assert_eq!(<Baz as Parse>::parse("42", ()), Ok(Baz(42f64)));
    }
}
