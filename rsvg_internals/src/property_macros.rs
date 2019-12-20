//! Macros to define CSS properties.

pub trait Property<T> {
    fn inherits_automatically() -> bool;
    fn compute(&self, _: &T) -> Self;
}

/// Generates a property definition that simply parses strings to enum variants
/// or to a tuple struct of the given type.
///
/// For example, the SVG spec defines the `stroke-linejoin` property
/// to have possible values `miter | round | bevel | inherit`, with a default
/// of `miter`.  We can define the property like this:
///
/// ```ignore
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
/// `impl Parse for StrokeLinejoin`, from `parsers::Parse`, with
/// `type Err = ValueErrorKind`.
#[macro_export]
macro_rules! make_property {
    ($computed_values_type: ty,
     $name: ident,
     default: $default: ident,
     inherits_automatically: $inherits_automatically: expr,
     identifiers:
     $($str_prop: expr => $variant: ident,)+
    ) => {
        #[derive(Debug, Copy, Clone, PartialEq)]
        pub enum $name {
            $($variant),+
        }

        impl_default!($name, $name::$default);
        impl_property!($computed_values_type, $name, $inherits_automatically);

        impl crate::parsers::ParseToParseError for $name {
            fn parse_to_parse_error<'i>(parser: &mut ::cssparser::Parser<'i, '_>) -> Result<$name, crate::error::CssParseError<'i>> {
                Ok(parse_identifiers!(
                    parser,
                    $($str_prop => $name::$variant,)+
                )?)
            }
        }
    };

    ($computed_values_type: ty,
     $name: ident,
     default: $default: expr,
     inherits_automatically: $inherits_automatically: expr,
     newtype_parse: $type: ty,
    ) => {
        #[derive(Debug, Clone, PartialEq)]
        pub struct $name(pub $type);

        impl_default!($name, $name($default));
        impl_property!($computed_values_type, $name, $inherits_automatically);

        impl crate::parsers::Parse for $name {
            fn parse(parser: &mut ::cssparser::Parser<'_, '_>) -> Result<$name, crate::error::ValueErrorKind> {
                Ok($name(<$type as crate::parsers::Parse>::parse(parser)?))
            }
        }
    };

    ($computed_values_type: ty,
     $name: ident,
     default: $default: expr,
     inherits_automatically: $inherits_automatically: expr,
     newtype_parse_to_parse_error: $type: ty,
    ) => {
        #[derive(Debug, Clone, PartialEq)]
        pub struct $name(pub $type);

        impl_default!($name, $name($default));
        impl_property!($computed_values_type, $name, $inherits_automatically);

        impl crate::parsers::ParseToParseError for $name {
            fn parse_to_parse_error<'i>(parser: &mut ::cssparser::Parser<'i, '_>) -> Result<$name, crate::error::CssParseError<'i>> {
                Ok($name(<$type as crate::parsers::ParseToParseError>::parse_to_parse_error(parser)?))
            }
        }
    };

    ($computed_values_type: ty,
     $name: ident,
     default: $default: expr,
     newtype_parse: $type: ty,
     property_impl: { $prop: item }
    ) => {
        #[derive(Debug, Clone, PartialEq)]
        pub struct $name(pub $type);

        impl_default!($name, $name($default));

        $prop

        impl crate::parsers::ParseToParseError for $name {
            fn parse_to_parse_error<'i>(parser: &mut ::cssparser::Parser<'i, '_>) -> Result<$name, crate::error::CssParseError<'i>> {
                Ok($name(<$type as crate::parsers::ParseToParseError>::parse_to_parse_error(parser)?))
            }
        }
    };

    ($computed_values_type: ty,
     $name: ident,
     default: $default: expr,
     newtype: $type: ty,
     property_impl: { $prop: item },
     parse_impl: { $parse: item }
    ) => {
        #[derive(Debug, Clone, PartialEq)]
        pub struct $name(pub $type);

        impl_default!($name, $name($default));

        $prop

        $parse
    };

    ($computed_values_type: ty,
     $name: ident,
     default: $default: expr,
     inherits_automatically: $inherits_automatically: expr,
     newtype: $type: ty,
     parse_impl: { $parse: item },
    ) => {
        #[derive(Debug, Clone, PartialEq)]
        pub struct $name(pub $type);

        impl_default!($name, $name($default));
        impl_property!($computed_values_type, $name, $inherits_automatically);

        $parse
    };

    ($computed_values_type: ty,
     $name: ident,
     inherits_automatically: $inherits_automatically: expr,
     fields: {
       $($field_name: ident : $field_type: ty, default: $field_default : expr,)+
     }
     parse_impl: { $parse: item }
    ) => {
        #[derive(Debug, Clone, PartialEq)]
        pub struct $name {
            $(pub $field_name: $field_type),+
        }

        impl_default!($name, $name { $($field_name: $field_default),+ });
        impl_property!($computed_values_type, $name, $inherits_automatically);

        $parse
    };
}

macro_rules! impl_default {
    ($name:ident, $default:expr) => {
        impl Default for $name {
            fn default() -> $name {
                $default
            }
        }
    };
}

macro_rules! impl_property {
    ($computed_values_type:ty, $name:ident, $inherits_automatically:expr) => {
        impl crate::property_macros::Property<$computed_values_type> for $name {
            fn inherits_automatically() -> bool {
                $inherits_automatically
            }

            fn compute(&self, _v: &$computed_values_type) -> Self {
                self.clone()
            }
        }
    };
}

#[cfg(test)]
mod tests {
    use super::*;

    use crate::parsers::ParseToParseError;
    use cssparser::RGBA;

    #[test]
    fn check_identifiers_property() {
        make_property! {
            (),
            Foo,
            default: Def,
            inherits_automatically: true,

            identifiers:
            "def" => Def,
            "bar" => Bar,
            "baz" => Baz,
        }

        assert_eq!(<Foo as Default>::default(), Foo::Def);
        assert_eq!(<Foo as Property<()>>::inherits_automatically(), true);
        assert!(<Foo as ParseToParseError>::parse_str_to_parse_error("blargh").is_err());
        assert_eq!(<Foo as ParseToParseError>::parse_str_to_parse_error("bar"), Ok(Foo::Bar));
    }

    #[test]
    fn check_compute() {
        make_property! {
            RGBA,
            AddColor,
            default: RGBA::new(1, 1, 1, 1),
            newtype_parse: RGBA,
            property_impl: {
                impl Property<RGBA> for AddColor {
                    fn inherits_automatically() -> bool {
                        true
                    }

                    fn compute(&self, v: &RGBA) -> Self {
                        AddColor(RGBA::new(
                            self.0.red + v.red,
                            self.0.green + v.green,
                            self.0.blue + v.blue,
                            self.0.alpha + v.alpha
                        ))
                    }
                }
            }
        }

        let color = RGBA::new(1, 1, 1, 1);
        let a = <AddColor as ParseToParseError>::parse_str_to_parse_error("#02030405").unwrap();
        let b = a.compute(&color);

        assert_eq!(b, AddColor(RGBA::new(3, 4, 5, 6)));
    }
}
