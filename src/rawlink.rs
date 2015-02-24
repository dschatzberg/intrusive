// Copyright 2012-2015 The Rust Project Developers. See the COPYRIGHT
// file at the top-level directory of this distribution and at
// http://rust-lang.org/COPYRIGHT.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.
use core::prelude::*;
use core::default::Default;
use core::mem;
use core::ptr;

pub struct Rawlink<T> {
    p: *mut T
}

impl<T> Copy for Rawlink<T> {}
unsafe impl<T:'static+Send> Send for Rawlink<T> {}
unsafe impl<T:Send+Sync> Sync for Rawlink<T> {}

/// Rawlink is a type like Option<T> but for holding a raw pointer
impl<T> Rawlink<T> {
    /// Like Option::None for Rawlink
    pub fn none() -> Rawlink<T> {
        Rawlink{p: ptr::null_mut()}
    }

    /// Like Option::Some for Rawlink
    pub fn some(n: &mut T) -> Rawlink<T> {
        Rawlink{p: n}
    }

    /// Convert the `Rawlink` into an Option value
    pub fn resolve_immut<'a>(&self) -> Option<&'a T> {
        unsafe {
            mem::transmute(self.p.as_ref())
        }
    }

    /// Convert the `Rawlink` into an Option value
    pub fn resolve<'a>(&mut self) -> Option<&'a mut T> {
        if self.p.is_null() {
            None
        } else {
            Some(unsafe { mem::transmute(self.p) })
        }
    }

    /// Return the `Rawlink` and replace with `Rawlink::none()`
    pub fn take(&mut self) -> Rawlink<T> {
        mem::replace(self, Rawlink::none())
    }
}

impl<T> Clone for Rawlink<T> {
    #[inline]
    fn clone(&self) -> Rawlink<T> {
        Rawlink{p: self.p}
    }
}

impl<T> Default for Rawlink<T> {
    fn default() -> Rawlink<T> { Rawlink::none() }
}
