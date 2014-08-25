// This file is part of Intrusive.

// Intrusive is free software: you can redistribute it and/or modify
// it under the terms of the GNU Lesser General Public License as published by
// the Free Software Foundation, either version 3 of the License, or
// (at your option) any later version.

// Intrusive is distributed in the hope that it will be useful,
// but WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
// GNU Lesser General Public License for more details.

// You should have received a copy of the GNU Lesser General Public License
// along with Intrusive.  If not, see <http://www.gnu.org/licenses/>.

#![crate_name = "intrusive"]
#![experimental]
#![crate_type = "rlib"]
#![license = "LGPL3"]
#![feature(globs,phase,unsafe_destructor)]
#![no_std]

#[phase(plugin, link)] extern crate core;

#[cfg(test)] extern crate native;
#[cfg(test)] extern crate test;
#[cfg(test)] extern crate debug;

#[cfg(test)] #[phase(plugin, link)] extern crate std;
#[cfg(test)] #[phase(plugin, link)] extern crate log;

pub use dlist::DList;

use core::collections::Collection;

pub mod dlist;

pub trait Mutable : Collection {
    /// Clears the container, removing all values.
    fn clear(&mut self);
}

pub trait MutableSeq<T> : Mutable {
    /// Appends an element to the back of a collection.
    ///
    /// # Safety Notes
    ///
    /// Inserting an element into an intrusive collection requires that the
    /// element lives longer than it will be in the container. If an element is
    /// dropped before it is removed, fail! will be called.
    fn push(&mut self, t: *mut T);

    /// Removes the last element from a collection and returns a pointer to it,
    /// The pointer is null if the collection is empty.
    fn pop(&mut self) -> *mut T;
}

/// A double-ended sequence that allows querying, insertion and deletion at both
/// ends
pub trait Deque<T> : MutableSeq<T> {
    /// Provides a pointer to the front element. The pointer is null if the
    /// sequence is empty.
    fn front(&self) -> *const T;

    /// Provides a mutable pointer to the front element. The pointer is null if
    /// the sequence is empty.
    fn front_mut(&mut self) -> *mut T;

    /// Provides a pointer to the last element. The pointer is null if the
    /// sequence is empty.
    fn back(&self) -> *const T;

    /// Provides a mutable pointer to the last element. The pointer is null if
    /// the sequence is empty.
    fn back_mut(&mut self) -> *mut T;

    /// Inserts an element first in the sequence
    ///
    /// # Safety Notes
    ///
    /// Inserting an element into an intrusive collection requires that the
    /// element lives longer than it will be in the container. If an element is
    /// dropped before it is removed, fail! will be called.
    fn push_front(&mut self, *mut T);

    /// Removes the first element and returns a pointer to it, the pointer is
    /// null if the sequence is empty
    fn pop_front(&mut self) -> *mut T;
}

#[cfg(not(test))]
mod std {
    pub use core::fmt;
    pub use core::option;
    pub use core::clone;
    pub use core::cmp;
}
