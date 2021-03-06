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

#![crate_name = "intrusive_containers"]
#![crate_type = "rlib"]
#![feature(core)]
#![cfg_attr(all(feature="nostd",not(test)), feature(no_std))]
#![cfg_attr(all(feature="nostd",not(test)), no_std)]
#![cfg_attr(any(not(feature="nostd"),test), feature(alloc))]
#![cfg_attr(test, feature(collections, hash, test))]

#[cfg(all(feature="nostd",not(test)))] #[macro_use] extern crate core as std;
#[cfg(all(feature="nostd",not(test)))] #[macro_use] extern crate core;

#[cfg(test)] extern crate test;
#[cfg(test)] extern crate rand;

pub use linked_list::LinkedList;

pub mod linked_list;

mod rawlink;
