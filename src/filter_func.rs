use crate::filters::{FilterResolveError, FilterSpec};

/// CSS Filter functions from the Filter Effects Module Level 1
///
/// https://www.w3.org/TR/filter-effects/#filter-functions
#[derive(Debug, Clone, PartialEq)]
pub enum FilterFunction {
}

impl FilterFunction {
    pub fn to_filter_spec(&self) -> Result<FilterSpec, FilterResolveError> {
        unimplemented!()
    }
}
