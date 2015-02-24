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

//! An intrusive double-linked list.
//!
//! The 'LinkedList' allows elements to be inserted or removed from either end.
use core::default::Default;
use core::marker::PhantomData;
use core::ops::Deref;
use core::ops::DerefMut;
use core::prelude::*;
use super::rawlink::Rawlink;

/// An intrusive doubly-linked list
pub struct LinkedList<T, L, LP>
    where LP: DerefMut<Target=Sentinel<T, L>>
{
    length: usize,
    sentinel: LP,
}

pub struct Sentinel<T, L>
    where L: Linkable<T>
{
    links: L,
    _marker: PhantomData<T>,
}

#[unsafe_destructor]
impl<T, L> Drop for Sentinel<T, L>
    where T: DerefMut,
          <T as Deref>::Target: Node<T, L>,
          L: Linkable<T> + 'static // the static is here due to rust issue #22062
{
    fn drop(&mut self) {
        self.clear();
    }
}

impl<T, L> Sentinel<T, L>
    where T: DerefMut,
          <T as Deref>::Target: Node<T, L>,
          L: Linkable<T> + 'static // the static is here due to rust issue #22062
{
    #[inline]
    pub fn new(l: L) -> Sentinel<T, L> {
        Sentinel { links: l, _marker: PhantomData }
    }

    fn clear(&mut self) {
        // remove the elements using a loop, ensuring not to have a recursive
        // destruction
        while let Some(mut link) = self.links.get_next_mut().take() {
            *self.links.get_next_mut() = link.get_next_mut().take();
            link.get_pprev_mut().take();
        }

        *self.links.get_pprev_mut() = Rawlink::none();
    }

    fn get_head(&self) -> &Link<T> { self.links.get_next() }
    fn get_head_mut(&mut self) -> &mut Link<T> {
        self.links.get_next_mut()
    }
    fn get_ptail(&self) -> &Rawlink<L> {
        self.links.get_pprev()
    }
    fn get_ptail_mut(&mut self) -> &mut Rawlink<L> {
        self.links.get_pprev_mut()
    }
}

type Link<T> = Option<T>;

/// Link trait allowing a struct to be inserted into a `LinkedList`
///
/// The trait is unsafe because any implementation must impl Drop to call
/// check_links()
pub unsafe trait Linkable<T> : Sized
{
    fn get_next(&self) -> &Link<T>;
    fn get_next_mut(&mut self) -> &mut Link<T>;
    fn get_pprev(&self) -> &Rawlink<Self>;
    fn get_pprev_mut(&mut self) -> &mut Rawlink<Self>;
    fn check_links(&self) {
        assert!(self.get_next().is_none());
        assert!(self.get_pprev().resolve_immut().is_none());
    }
}

#[derive(Clone)]
pub struct Links<T> {
    next: Link<T>,
    pprev: Rawlink<Links<T>>,
}

impl<T> Default for Links<T> {
    fn default() -> Links<T> {
        Links { next: None, pprev: Rawlink::none() }
    }
}

#[unsafe_destructor]
impl<T> Drop for Links<T> {
    fn drop(&mut self) {
        self.check_links()
    }
}

unsafe impl<T> Linkable<T> for Links<T> {
    fn get_next(&self) -> &Link<T> { &self.next }

    fn get_next_mut(&mut self) -> &mut Link<T> { &mut self.next }

    fn get_pprev(&self) -> &Rawlink<Self> { &self.pprev }

    fn get_pprev_mut(&mut self) -> &mut Rawlink<Self> { &mut self.pprev }
}

impl<T> Links<T> {
    #[inline]
    pub fn new() -> Links<T> { Default::default() }
}

/// A trait that allows a struct to be inserted into a `LinkedList`
pub trait Node<T, L>
    where L: Linkable<T> + 'static // the static is here due to rust issue #22062
{
    /// Getter for links
    fn get_list_hook(&self) -> &L;

    /// Getter for mutable links
    fn get_list_hook_mut(&mut self) -> &mut L;

    fn get_next(&self) -> &Link<T> {
        &self.get_list_hook().get_next()
    }

    fn get_next_mut(&mut self) -> &mut Link<T> {
        self.get_list_hook_mut().get_next_mut()
    }

    fn get_pprev(&self) -> &Rawlink<L> {
        &self.get_list_hook().get_pprev()
    }

    fn get_pprev_mut(&mut self) -> &mut Rawlink<L> {
        self.get_list_hook_mut().get_pprev_mut()
    }
}

impl<T, L, LP> LinkedList<T, L, LP>
    where T: DerefMut,
          <T as Deref>::Target: Node<T, L>,
          L: Linkable<T> + 'static, // the static is here due to rust issue #22062
          LP: DerefMut<Target=Sentinel<T, L>>
{
    /// Creates an empty `LinkedList`
    #[inline]
    pub fn new(mut sentinel: LP) -> LinkedList<T, L, LP> {
        sentinel.clear();
        LinkedList{length: 0, sentinel: sentinel}
    }

    /// Returns `true` if the `LinkedList` is empty
    ///
    /// This operation should compute in O(1) time
    #[inline]
    pub fn is_empty(&self) -> bool {
        self.length == 0
    }

    /// Returns the length of the `LinkedList`.
    ///
    /// This operation should compute in O(1) time.
    #[inline]
    pub fn len(&self) -> usize {
        self.length
    }

    /// Provides a reference to the front element, or `None` if the list is
    /// empty.
    #[inline]
    pub fn front(&self) -> Option<&T> {
        self.sentinel.get_head().as_ref()
    }

    /// Provides a mutable reference to the front element, or `None` if the list
    /// is empty.
    #[inline]
    pub fn front_mut(&mut self) -> Option<&mut T> {
        self.sentinel.get_head_mut().as_mut()
    }

    /// Provides a reference to the back element, or `None` if the list is
    /// empty.
    #[inline]
    pub fn back(&self) -> Option<&T> {
        // if pprev is not none, then it points to the link before the tail
        self.sentinel.get_ptail().resolve_immut().map(|link| {
            link.get_next().as_ref().unwrap()
        })
    }

    /// Provides a mutable reference to the back element, or `None` if the list
    /// is empty.
    #[inline]
    pub fn back_mut(&mut self) -> Option<&mut T> {
        // if pprev is not none, then it points to the link before the tail
        self.sentinel.get_ptail_mut().resolve().map(|mut link| {
            link.get_next_mut().as_mut().unwrap()
        })
    }

    /// Adds an element first in the list.
    ///
    /// This operation should compute in O(1) time.
    pub fn push_front(&mut self, mut elt: T) {
        // ensure links are not already being used
        elt.get_list_hook().check_links();

        if self.is_empty() {
            // Point tail to sentinel
            *self.sentinel.get_ptail_mut() =
                Rawlink::some(&mut self.sentinel.links);
        } else {
            if self.len() == 1 {
                // need to advance tail pointer
                *self.sentinel.get_ptail_mut() =
                    Rawlink::some(elt.get_list_hook_mut());
            }
            // Chain head backwards to elt
            *self.front_mut().unwrap().get_pprev_mut() =
                Rawlink::some(elt.get_list_hook_mut());
        }
        // Chain elt into the list
        *elt.get_next_mut() = self.sentinel.get_head_mut().take();
        *elt.get_pprev_mut() = Rawlink::some(&mut self.sentinel.links);
        // Set it as the new head
        *self.sentinel.get_head_mut() = Some(elt);
        self.length += 1;
    }

    /// Removes the first element and returns it, or `None` if the list is
    /// empty.
    ///
    /// This operation should compute in O(1) time.
    pub fn pop_front(&mut self) -> Option<T> {
        self.sentinel.get_head_mut().take().map(|mut front| {
            self.length -= 1;
            front.get_pprev_mut().take();
            let mut next_link = front.get_next_mut().take();
            if let Some(ref mut next) = next_link {
                // chain the following element to the list
                *next.get_pprev_mut() = Rawlink::some(&mut self.sentinel.links);
            } else {
                // the list will be empty, clear the tail pointer
                *self.sentinel.get_ptail_mut() = Rawlink::none();
            }
            *self.sentinel.get_head_mut() = next_link;
            front
        })
    }

    /// Appends an element to the back of a list
    ///
    /// This operation should compute in O(1) time.
    pub fn push_back(&mut self, mut elt: T) {
        // ensure links are not already being used
        elt.get_list_hook().check_links();

        match self.sentinel.get_ptail_mut().resolve() {
            None => return self.push_front(elt),
            Some(ref mut ptail) => {
                let mut tail = ptail.get_next_mut().as_mut().unwrap();
                *elt.get_pprev_mut() = Rawlink::some(tail.get_list_hook_mut());
                *tail.get_next_mut() = Some(elt);
                // advance the ptail pointer in the sentinel
                *self.sentinel.get_ptail_mut() =
                    Rawlink::some(tail.get_list_hook_mut());
                self.length += 1;
            }
        }
    }

    /// Removes the last element from a list and returns it, or `None` if
    /// it is empty.
    ///
    /// This operation should compute in O(1) time
    pub fn pop_back(&mut self) -> Option<T> {
        if self.len() <= 1 { return self.pop_front(); }
        self.length -= 1;
        let mut ptail = self.sentinel.get_ptail_mut().resolve().take().unwrap();
        let mut tail = ptail.get_next_mut().take().unwrap();
        // clear the links
        debug_assert!(tail.get_next().is_none());
        tail.get_pprev_mut().take();
        // step the tail pointer back
        *self.sentinel.get_ptail_mut() = *ptail.get_pprev();
        Some(tail)
    }
}

#[cfg(test)]
mod tests {
    use std::prelude::v1::*;
    use std::default::Default;
    use std::fmt;
    use super::{LinkedList, Links, Node, Sentinel};

    struct MyInt {
        links: Links<Box<MyInt>>,
        i: i32,
    }

    impl fmt::Debug for MyInt {
        fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
            write!(f, "MyInt({:?})", self.i)
        }
    }

    impl Node<Box<MyInt>, Links<Box<MyInt>>> for MyInt {
        fn get_list_hook(&self) -> &Links<Box<MyInt>> { &self.links }
        fn get_list_hook_mut(&mut self) -> &mut Links<Box<MyInt>> { &mut self.links }
    }

    impl PartialEq for MyInt {
        fn eq(&self, other: &MyInt) -> bool {
            self.i == other.i
        }
    }

    impl PartialEq<i32> for MyInt {
        fn eq(&self, other: &i32) -> bool {
            self.i == *other
        }
    }

    impl MyInt {
        pub fn new(i: i32) -> MyInt {
            MyInt { links: Default::default(), i: i}
        }
    }

    #[test]
    fn test_basic() {
        let sentinel = Box::new(Sentinel::new(Links::<Box<MyInt>>::new()));
        let mut m = LinkedList::new(sentinel);

        assert_eq!(m.pop_front(), None);
        assert_eq!(m.pop_back(), None);
        assert_eq!(m.pop_front(), None);
        m.push_front(Box::new(MyInt::new(1)));
        assert_eq!(m.pop_front(), Some(Box::new(MyInt::new(1))));
        m.push_back(Box::new(MyInt::new(2)));
        m.push_back(Box::new(MyInt::new(3)));
        assert_eq!(m.len(), 2);
        assert_eq!(m.pop_front(), Some(Box::new(MyInt::new(2))));
        assert_eq!(m.pop_front(), Some(Box::new(MyInt::new(3))));
        assert_eq!(m.len(), 0);
        assert_eq!(m.pop_front(), None);
        m.push_back(Box::new(MyInt::new(1)));
        m.push_back(Box::new(MyInt::new(3)));
        m.push_back(Box::new(MyInt::new(5)));
        m.push_back(Box::new(MyInt::new(7)));
        assert_eq!(m.pop_front(), Some(Box::new(MyInt::new(1))));


        let sentinel = Box::new(Sentinel::new(Links::<Box<MyInt>>::new()));
        let mut n = LinkedList::new(sentinel);
        n.push_front(Box::new(MyInt::new(2)));
        n.push_front(Box::new(MyInt::new(3)));
        {
            assert_eq!(n.front().unwrap().i, 3);
            let x = n.front_mut().unwrap();
            assert_eq!(x.i, 3);
            x.i = 0;
        }
        {
            assert_eq!(n.back().unwrap().i, 2);
            let y = n.back_mut().unwrap();
            assert_eq!(y.i, 2);
            y.i = 1;
        }
        assert_eq!(n.pop_front(), Some(Box::new(MyInt::new(0))));
        assert_eq!(n.pop_front(), Some(Box::new(MyInt::new(1))));
    }
}
