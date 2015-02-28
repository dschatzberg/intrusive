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

#![crate_name = "intrusive-containers"]
#![crate_type = "rlib"]
#![feature(box_syntax,core,no_std,unsafe_destructor, visible_private_types)]
#![cfg_attr(test, feature(collections, test))]
#![no_std]

#[macro_use] extern crate core;

#[cfg(test)] extern crate test;

#[cfg(test)] #[macro_use] extern crate std;

pub use linked_list::LinkedList;

pub mod linked_list;

mod rawlink;

#[cfg(not(test))]
mod std {
    pub use core::clone;
    pub use core::cmp;
    pub use core::fmt;
    pub use core::option;
}
