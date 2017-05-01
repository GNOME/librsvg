use std::mem;
use glib::translate::*;
use ffi;

#[derive(Clone, Copy, Debug, PartialEq)]
#[repr(C)]
pub struct DimensionData {
    pub width: i32,
    pub height: i32,
    pub em: f64,
    pub ex: f64,
}

impl DimensionData {
    pub fn new(width: i32, height: i32, em: f64, ex: f64) -> DimensionData {
        DimensionData {
            width: width,
            height: height,
            em: em,
            ex: ex,
        }
    }
}

#[doc(hidden)]
impl Uninitialized for DimensionData {
    #[inline]
    unsafe fn uninitialized() -> Self {
        mem::uninitialized()
    }
}

#[doc(hidden)]
impl<'a> ToGlibPtr<'a, *const ffi::RsvgDimensionData> for DimensionData {
    type Storage = &'a Self;

    #[inline]
    fn to_glib_none(&'a self) -> Stash<'a, *const ffi::RsvgDimensionData, Self> {
        let ptr: *const DimensionData = &*self;
        Stash(ptr as *const ffi::RsvgDimensionData, self)
    }
}

#[doc(hidden)]
impl<'a> ToGlibPtrMut<'a, *mut ffi::RsvgDimensionData> for DimensionData {
    type Storage = &'a mut Self;

    #[inline]
    fn to_glib_none_mut(&'a mut self) -> StashMut<'a, *mut ffi::RsvgDimensionData, Self> {
        let ptr: *mut DimensionData = &mut *self;
        StashMut(ptr as *mut ffi::RsvgDimensionData, self)
    }
}

#[doc(hidden)]
impl FromGlibPtrNone<*const ffi::RsvgDimensionData> for DimensionData {
    unsafe fn from_glib_none(ptr: *const ffi::RsvgDimensionData) -> Self {
        *(ptr as *const DimensionData)
    }
}

#[doc(hidden)]
impl FromGlibPtrNone<*mut ffi::RsvgDimensionData> for DimensionData {
    unsafe fn from_glib_none(ptr: *mut ffi::RsvgDimensionData) -> Self {
        *(ptr as *mut DimensionData)
    }
}