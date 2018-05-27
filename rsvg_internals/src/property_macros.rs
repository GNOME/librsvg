pub trait Property<T> {
    fn inherits_automatically() -> bool;
    fn compute(&self, &T) -> Self;
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

    ($computed_values_type: ty,
     $name: ident,
     default: $default: expr,
     inherits_automatically: $inherits_automatically: expr,
     newtype_from_str: $type: ty
    ) => {
        #[derive(Debug, Clone, PartialEq)]
        pub struct $name(pub $type);

        impl_default!($name, $name($default));
        impl_property!($computed_values_type, $name, $inherits_automatically);

        impl ::parsers::Parse for $name {
            type Data = ();
            type Err = ::error::AttributeError;

            fn parse(s: &str, _: Self::Data) -> Result<$name, ::error::AttributeError> {
                s.trim()
                    .parse()
                    .map(|val| $name(val))
                    .map_err(|_| ::error::AttributeError::from(
                        ::parsers::ParseError::new("parse error")
                    ))
            }
        }
    };

    ($computed_values_type: ty,
     $name: ident,
     default: $default: expr,
     inherits_automatically: $inherits_automatically: expr,
     newtype_parse: $type: ty,
     parse_data_type: $parse_data_type: ty
    ) => {
        #[derive(Debug, Clone, PartialEq)]
        pub struct $name(pub $type);

        impl_default!($name, $name($default));

        impl ::property_macros::Property<$computed_values_type> for $name {
            fn inherits_automatically() -> bool {
                $inherits_automatically
            }

            fn compute(&self, _v: &$computed_values_type) -> Self {
                self.clone()
            }
        }

        impl ::parsers::Parse for $name {
            type Data = $parse_data_type;
            type Err = ::error::AttributeError;

            fn parse(s: &str, d: Self::Data) -> Result<$name, ::error::AttributeError> {
                Ok($name(<$type as ::parsers::Parse>::parse(s, d)?))
            }
        }
    };

    ($computed_values_type: ty,
     $name: ident,
     default: $default: expr,
     newtype_parse: $type: ty,
     parse_data_type: $parse_data_type: ty,
     property_impl: { $prop: item }
    ) => {
        #[derive(Debug, Clone, PartialEq)]
        pub struct $name(pub $type);

        impl_default!($name, $name($default));

        $prop

        impl ::parsers::Parse for $name {
            type Data = $parse_data_type;
            type Err = ::error::AttributeError;

            fn parse(s: &str, d: Self::Data) -> Result<$name, ::error::AttributeError> {
                Ok($name(<$type as ::parsers::Parse>::parse(s, d)?))
            }
        }
    };

    ($computed_values_type: ty,
     $name: ident,
     default: $default: expr,
     inherits_automatically: $inherits_automatically: expr,
     newtype: $type: ty,
     parse_impl: { $parse: item }
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
    ($computed_values_type: ty, $name:ident, $inherits_automatically:expr) => {
        impl ::property_macros::Property<$computed_values_type> for $name {
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

    use cssparser::RGBA;
    use parsers::Parse;

    #[test]
    fn check_generated_property() {
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
        assert!(<Foo as Parse>::parse("blargh", ()).is_err());
        assert_eq!(<Foo as Parse>::parse("bar", ()), Ok(Foo::Bar));

        make_property! {
            (),
            Bar,
            default: "bar".to_string(),
            inherits_automatically: true,
            newtype_from_str: String
        }

        assert_eq!(<Bar as Default>::default(), Bar("bar".to_string()));
        assert_eq!(<Bar as Property<()>>::inherits_automatically(), true);
        assert_eq!(
            <Bar as Parse>::parse("test", ()),
            Ok(Bar("test".to_string()))
        );

        make_property! {
            (),
            Baz,
            default: 42f64,
            inherits_automatically: true,
            newtype_from_str: f64
        }

        assert_eq!(<Baz as Default>::default(), Baz(42f64));
        assert_eq!(<Baz as Property<()>>::inherits_automatically(), true);
        assert_eq!(<Baz as Parse>::parse("42", ()), Ok(Baz(42f64)));
    }

    #[test]
    fn check_compute() {
        make_property! {
            RGBA,
            AddColor,
            default: RGBA::new(1, 1, 1, 1),
            newtype_parse: RGBA,
            parse_data_type: (),
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
        let a = <AddColor as Parse>::parse("#02030405", ()).unwrap();
        let b = a.compute(&color);

        assert_eq!(b, AddColor(RGBA::new(3, 4, 5, 6)));
    }
}
