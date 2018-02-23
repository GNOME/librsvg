use std::mem;
use glib::translate::*;
use ffi;

#[derive(Clone, Copy, Debug, PartialEq)]
#[repr(C)]
pub struct PositionData {
    pub x: i32,
    pub y: i32,
    pub em: f64,
    pub ex: f64,
}

impl PositionData {
    pub fn new(x: i32, y: i32, em: f64, ex: f64) -> PositionData {
        PositionData {
            x: x,
            y: y,
            em: em,
            ex: ex,
        }
    }
}

#[doc(hidden)]
impl Uninitialized for PositionData {
    #[inline]
    unsafe fn uninitialized() -> Self {
        mem::uninitialized()
    }
}

#[doc(hidden)]
impl<'a> ToGlibPtr<'a, *const ffi::RsvgPositionData> for PositionData {
    type Storage = &'a Self;

    #[inline]
    fn to_glib_none(&'a self) -> Stash<'a, *const ffi::RsvgPositionData, Self> {
        let ptr: *const PositionData = &*self;
        Stash(ptr as *const ffi::RsvgPositionData, self)
    }
}

#[doc(hidden)]
impl<'a> ToGlibPtrMut<'a, *mut ffi::RsvgPositionData> for PositionData {
    type Storage = &'a mut Self;

    #[inline]
    fn to_glib_none_mut(&'a mut self) -> StashMut<'a, *mut ffi::RsvgPositionData, Self> {
        let ptr: *mut PositionData = &mut *self;
        StashMut(ptr as *mut ffi::RsvgPositionData, self)
    }
}

#[doc(hidden)]
impl FromGlibPtrNone<*const ffi::RsvgPositionData> for PositionData {
    unsafe fn from_glib_none(ptr: *const ffi::RsvgPositionData) -> Self {
        *(ptr as *const PositionData)
    }
}

#[doc(hidden)]
impl FromGlibPtrNone<*mut ffi::RsvgPositionData> for PositionData {
    unsafe fn from_glib_none(ptr: *mut ffi::RsvgPositionData) -> Self {
        *(ptr as *mut PositionData)
    }
}