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
        #[repr(C)]
        pub enum $name {
            $($variant),+
        }

        impl_default!($name, $name::$default);
        impl_property!($computed_values_type, $name, $inherits_automatically);

        impl crate::parsers::Parse for $name {
            fn parse<'i>(parser: &mut ::cssparser::Parser<'i, '_>) -> Result<$name, crate::error::ParseError<'i>> {
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
            fn parse<'i>(parser: &mut ::cssparser::Parser<'i, '_>) -> Result<$name, crate::error::ParseError<'i>> {
                Ok($name(<$type as crate::parsers::Parse>::parse(parser)?))
            }
        }
    };

    ($computed_values_type: ty,
     $name: ident,
     default: $default: expr,
     property_impl: { $prop: item }
    ) => {
        impl_default!($name, $default);

        $prop
    };

    // pending - only BaselineShift
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

    use crate::parsers::Parse;

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
        assert!(<Foo as Parse>::parse_str("blargh").is_err());
        assert_eq!(<Foo as Parse>::parse_str("bar"), Ok(Foo::Bar));
    }
}
