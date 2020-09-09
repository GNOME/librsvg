//! Macros to define CSS properties.

/// Trait which all CSS property types should implement.
///
/// This is generic on `T` for testing purposes; in the actual code `T` needs to
/// be [`ComputedValues`].
///
/// [`ComputedValues`]: ../properties/struct.ComputedValues.html
pub trait Property<T> {
    /// Whether the property's computed value inherits from parent to child elements.
    ///
    /// For each property, the CSS or SVG specs say whether the property inherits
    /// automatically.  When a property is not specified in an element, the return value
    /// of this method determines whether the property's value is copied from the parent
    /// element (`true`), or whether it resets to the initial/default value (`false`).
    fn inherits_automatically() -> bool;

    /// Derive the CSS computed value from the parent element's `ComputedValues` and the `self` value.
    ///
    /// The CSS or SVG specs say how to derive this for each property.
    fn compute(&self, _: &T) -> Self;
}

/// Generates a type for a CSS property.
///
/// Writing a property by hand takes a bit of boilerplate:
///
/// * Define a type to represent the property's values.
///
/// * A [`Parse`] implementation to parse the property.
///
/// * A `Default` implementation to define the property's *initial* value.
///
/// * A [`Property`] implementation to define whether the property
/// inherits from the parent element, and how the property derives
/// its computed value.
///
/// When going from [`SpecifiedValues`] to [`ComputedValues`], properties
/// which inherit automatically from the parent element will just have
/// their values cloned.  Properties which do not inherit will be reset back
/// to their initial value (i.e. their `Default`).
///
/// The default implementation of `Property::compute()` is to just
/// clone the property's value.  Properties which need more
/// sophisticated computation can override this.
///
/// This macro allows defining properties of different kinds; see the following
/// sections for examples.
///
/// # Simple identifiers
///
/// Many properties are just sets of identifiers and can be represented
/// by simple enums.  In this case, you can use the following:
///
/// ```ignore
/// make_property!(
///   ComputedValues,
///   StrokeLinejoin,
///   default: Miter,
///   inherits_automatically: true,
///
///   identifiers:
///     "miter" => Miter,
///     "round" => Round,
///     "bevel" => Bevel,
/// );
/// ```
///
/// This generates a simple enum like the following, with implementations of [`Parse`],
/// `Default`, and [`Property`].
///
/// ```ignore
/// pub enum StrokeLinejoin { Miter, Round, Bevel }
/// ```
///
/// # Properties from an existing, general-purpose type
///
/// For example, both the `lightingColor` and `floodColor` properties can be represented
/// with a `cssparser::Color`, but their intial values are different.  In this case, the macro
/// can generate a newtype around `cssparser::Color` for each case:
///
/// ```ignore
/// make_property!(
///     ComputedValues,
///     FloodColor,
///     default: cssparser::Color::RGBA(cssparser::RGBA::new(0, 0, 0, 0)),
///     inherits_automatically: false,
///     newtype_parse: cssparser::Color,
/// );
/// ```
///
/// # Properties from custom specific types
///
/// For example, font-related properties have custom, complex types that require an
/// implentation of `Property::compute` that is more than a simple `clone`.  In this case,
/// define the custom type separately, and use the macro to specify the default value and
/// the `Property` implementation.
///
/// [`Parse`]: ../trait.Parse.html
/// [`Property`]: ../property_macros/trait.Property.html
/// [`ComputedValues`]: ../properties/struct.ComputedValues.html
/// [`SpecifiedValues`]: ../properties/struct.SpecifiedValues.html
///
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

    ($computed_values_type: ty,
     $name: ident,
     default: $default: expr,
     inherits_automatically: $inherits_automatically: expr,
    ) => {
        impl_default!($name, $default);
        impl_property!($computed_values_type, $name, $inherits_automatically);
    };

    ($computed_values_type: ty,
     $name: ident,
     default: $default: expr,
     inherits_automatically: $inherits_automatically: expr,
     parse_impl: { $parse: item }
    ) => {
        impl_default!($name, $default);
        impl_property!($computed_values_type, $name, $inherits_automatically);

        $parse
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

    // pending - only XmlLang
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
