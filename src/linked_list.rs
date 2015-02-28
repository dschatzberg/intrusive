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
use core::iter::{FromIterator,IntoIterator};
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

impl<T, L> Default for Sentinel<T, L>
    where L: Linkable<T> + Default
{
    fn default() -> Sentinel<T, L> {
        Sentinel { links: Default::default(), _marker: PhantomData }
    }
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

#[derive(Clone, Debug)]
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
    fn get_links(&self) -> &L;

    /// Getter for mutable links
    fn get_links_mut(&mut self) -> &mut L;

    fn get_next(&self) -> &Link<T> {
        &self.get_links().get_next()
    }

    fn get_next_mut(&mut self) -> &mut Link<T> {
        self.get_links_mut().get_next_mut()
    }

    fn get_pprev(&self) -> &Rawlink<L> {
        &self.get_links().get_pprev()
    }

    fn get_pprev_mut(&mut self) -> &mut Rawlink<L> {
        self.get_links_mut().get_pprev_mut()
    }
}

/// An iterator over references to the items of a `LinkedList`
pub struct Iter<'a, T:'a, L: Linkable<T> + 'a> {
    head: &'a Link<T>,
    tail: &'a Rawlink<L>,
    nelem: usize,
}

impl<'a, T, L: Linkable<T>> Clone for Iter<'a, T, L> {
    fn clone(&self) -> Iter<'a, T, L> {
        Iter {
            head: self.head.clone(),
            tail: self.tail,
            nelem: self.nelem,
        }
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
    pub fn new_with_sentinel(mut sentinel: LP) -> LinkedList<T, L, LP> {
        sentinel.clear();
        LinkedList{length: 0, sentinel: sentinel}
    }

    /// Moves all elements from `other` to the end of the list.
    ///
    /// This reuses all the nodes from `other` and moves them into `self`. After
    /// this operation, `other` becomes empty.
    ///
    /// This operation should compute in O(1) time and O(1) memory.
    pub fn append<LPO>(&mut self, other: &mut LinkedList<T, L, LPO>)
        where LPO: DerefMut<Target=Sentinel<T, L>>
    {
        match self.sentinel.get_ptail_mut().resolve() {
            None => {
                self.length = other.length;
                if let Some(ref mut other_head) = *other.sentinel.get_head_mut() {
                    *other_head.get_pprev_mut() =
                        Rawlink::some(&mut self.sentinel.links);
                }
                *self.sentinel.get_head_mut() =
                    other.sentinel.get_head_mut().take();
                *self.sentinel.get_ptail_mut() = if other.length == 1 {
                    Rawlink::some(&mut self.sentinel.links)
                } else {
                    other.sentinel.get_ptail_mut().take()
                }
            },
            Some(mut ptail) => {
                let tail = ptail.get_next_mut().as_mut().unwrap();
                let o_tail = other.sentinel.get_ptail_mut().take();
                let o_length = other.length;
                match other.sentinel.get_head_mut().take() {
                    None => return,
                    Some(mut node) => {
                        *node.get_pprev_mut() =
                            Rawlink::some(tail.get_links_mut());
                        *tail.get_next_mut() = Some(node);
                        self.length += o_length;
                        *self.sentinel.get_ptail_mut() = if o_length == 1 {
                            Rawlink::some(tail.get_links_mut())
                        } else {
                            o_tail
                        }
                    }
                }
            }
        }
        other.length = 0;
    }


    /// Provides a forward iterator.
    #[inline]
    pub fn iter(&self) -> Iter<T, L> {
        Iter{nelem: self.len(), head: self.sentinel.get_head(),
             tail: self.sentinel.get_ptail()}
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
        elt.get_links().check_links();

        if self.is_empty() {
            // Point tail to sentinel
            *self.sentinel.get_ptail_mut() =
                Rawlink::some(&mut self.sentinel.links);
        } else {
            if self.len() == 1 {
                // need to advance tail pointer
                *self.sentinel.get_ptail_mut() =
                    Rawlink::some(elt.get_links_mut());
            }
            // Chain head backwards to elt
            *self.front_mut().unwrap().get_pprev_mut() =
                Rawlink::some(elt.get_links_mut());
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
        elt.get_links().check_links();

        match self.sentinel.get_ptail_mut().resolve() {
            None => return self.push_front(elt),
            Some(ref mut ptail) => {
                let mut tail = ptail.get_next_mut().as_mut().unwrap();
                *elt.get_pprev_mut() = Rawlink::some(tail.get_links_mut());
                *tail.get_next_mut() = Some(elt);
                // advance the ptail pointer in the sentinel
                *self.sentinel.get_ptail_mut() =
                    Rawlink::some(tail.get_links_mut());
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

impl<T, L, LP> Default for LinkedList<T, L, LP>
    where T: DerefMut,
          <T as Deref>::Target: Node<T, L>,
          L: Linkable<T> + Default + 'static,
          LP: DerefMut<Target=Sentinel<T, L>> + Default
{
    #[inline]
    fn default() -> LinkedList<T, L, LP> {
        LinkedList::new_with_sentinel(Default::default())
    }
}

impl<T, L, LP> LinkedList<T, L, LP>
    where T: DerefMut,
          <T as Deref>::Target: Node<T, L>,
          L: Linkable<T> + Default + 'static,
          LP: DerefMut<Target=Sentinel<T, L>> + Default
{
    #[inline]
    pub fn new() -> LinkedList<T, L, LP> {
        LinkedList::new_with_sentinel(Default::default())
    }

    // /// Splits the list into two at the given index. Returns everything after the given index,
    // /// including the index.
    // ///
    // /// # Panics
    // ///
    // /// Panics if `at > len`.
    // ///
    // /// This operation should compute in O(n) time.
    // pub fn split_off(&mut self, at: usize) -> LinkedList<T, L, LP> {
    //     let len = self.len();
    //     assert!(at <= len, "Cannot split off at a nonexistent index");
    //     if at == 0 {
    //         return mem::replace(self, LinkedList::new());
    //     } else if at == len {
    //         return LinkedList::new();
    //     }

    //     let mut iter = self.iter_mut();
    //     // instead of skipping using .skip() (which creates a new struct),
    //     // we skip manually so we can access the head field without
    //     // depending on implementation details of Skip
    //     for _ in 0..at - 1 {
    //         iter.next();
    //     }
    //     let mut split_node = iter.head;

    //     let mut splitted_list = LinkedList {
    //         sentinel: Default::default();
    //         length: len - at
    //     }

    //     mem::swap(&mut split_node.resolve().unwrap().next, &mut splitted_list.list_head);
    //     self.list_tail = split_node;
    //     self.length = at;

    //     splitted_list
    // }
}


impl<T, L, LP> Extend<T> for LinkedList<T, L, LP>
    where T: DerefMut,
          <T as Deref>::Target: Node<T, L>,
          L: Linkable<T> + 'static, // the static is here due to rust issue #22062
          LP: DerefMut<Target=Sentinel<T, L>>
{
    fn extend<I: IntoIterator<Item=T>>(&mut self, iter: I) {
        for elt in iter { self.push_back(elt); }
    }
}

impl<T, L, LP> FromIterator<T> for LinkedList<T, L, LP>
    where T: DerefMut,
          <T as Deref>::Target: Node<T, L>,
          L: Linkable<T> + Default + 'static,
          LP: DerefMut<Target=Sentinel<T, L>> + Default
{
    fn from_iter<I: IntoIterator<Item=T>>(iter: I) -> LinkedList<T, L, LP> {
        let mut ret = LinkedList::new();
        ret.extend(iter);
        ret
    }
}

impl<'a, T, L: Linkable<T>> Iterator for Iter<'a, T, L>
    where T: DerefMut + 'a,
          <T as Deref>::Target: Node<T, L> + 'a,
          L: Linkable<T> + 'static // the static is here due to rust issue #22062
{
    type Item = &'a T;

    #[inline]
    fn next(&mut self) -> Option<&'a T> {
        if self.nelem == 0 {
            return None;
        }
        self.head.as_ref().map(|head| {
            self.nelem -= 1;
            self.head = head.get_next();
            head
        })
    }

    #[inline]
    fn size_hint(&self) -> (usize, Option<usize>) {
        (self.nelem, Some(self.nelem))
    }
}

impl<'a, T, L: Linkable<T>> DoubleEndedIterator for Iter<'a, T, L>
    where T: DerefMut + 'a,
          <T as Deref>::Target: Node<T, L> + 'a,
          L: Linkable<T> + 'static // the static is here due to rust issue #22062
{
    #[inline]
    fn next_back(&mut self) -> Option<&'a T> {
        if self.nelem == 0 {
            return None;
        }
        self.tail.resolve_immut().as_ref().map(|prev| {
            self.nelem -= 1;
            self.tail = prev.get_pprev();
            prev.get_next().as_ref().unwrap()
        })
    }
}

#[cfg(test)]
mod tests {
    use std::prelude::v1::*;
    use std::default::Default;
    use std::fmt;
    use std::ops::{Deref, DerefMut};
    use super::{LinkedList, Linkable, Links, Node, Sentinel};

    struct MyInt {
        links: Links<Box<MyInt>>,
        i: i32,
    }

    impl Clone for MyInt {
        fn clone(&self) -> MyInt {
            MyInt::new(self.i)
        }
    }

    impl fmt::Debug for MyInt {
        fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
            write!(f, "MyInt({:?})", self.i)
        }
    }

    impl Node<Box<MyInt>, Links<Box<MyInt>>> for MyInt {
        fn get_links(&self) -> &Links<Box<MyInt>> { &self.links }
        fn get_links_mut(&mut self) -> &mut Links<Box<MyInt>> { &mut self.links }
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

    type ListAlias<T> = LinkedList<T, Links<T>, Box<Sentinel<T, Links<T>>>>;
    type MyIntList = ListAlias<Box<MyInt>>;

    pub fn check_links<T, L, LP>(list: &LinkedList<T, L, LP>)
        where T: DerefMut,
              <T as Deref>::Target: Node<T, L>,
              L: Linkable<T> + fmt::Debug + 'static, // the static is here due
                                                     // to rust issue #22062
              LP: DerefMut<Target=Sentinel<T, L>>
    {
        let mut len = 0;
        let mut last_link = &list.sentinel.links;
        let mut node_ptr: &<T as Deref>::Target;
        match *list.sentinel.get_head() {
            None => { assert_eq!(0, list.length); return }
            Some(ref node) => node_ptr = &**node,
        }
        loop {
            match node_ptr.get_pprev().resolve_immut() {
                None => panic!("unset prev link"),
                Some(pprev) => {
                    assert_eq!(last_link as *const L, pprev as *const L);
                }
            }
            match *node_ptr.get_next() {
                Some(ref next) => {
                    last_link = node_ptr.get_links();
                    node_ptr = &**next;
                    len += 1;
                }
                None => {
                    assert_eq!(node_ptr.get_pprev(),
                               list.sentinel.get_ptail());
                    len += 1;
                    break;
                }
            }
        }
        assert_eq!(len, list.length);
    }

    #[test]
    fn test_basic() {
        let mut m: MyIntList = LinkedList::new();

        assert_eq!(m.pop_front(), None);
        assert_eq!(m.pop_back(), None);
        assert_eq!(m.pop_front(), None);
        m.push_front(box MyInt::new(1));
        assert_eq!(m.pop_front(), Some(box MyInt::new(1)));
        m.push_back(box MyInt::new(2));
        m.push_back(box MyInt::new(3));
        assert_eq!(m.len(), 2);
        assert_eq!(m.pop_front(), Some(box MyInt::new(2)));
        assert_eq!(m.pop_front(), Some(box MyInt::new(3)));
        assert_eq!(m.len(), 0);
        assert_eq!(m.pop_front(), None);
        m.push_back(box MyInt::new(1));
        m.push_back(box MyInt::new(3));
        m.push_back(box MyInt::new(5));
        m.push_back(box MyInt::new(7));
        assert_eq!(m.pop_front(), Some(box MyInt::new(1)));


        let mut n: MyIntList = LinkedList::new();
        n.push_front(box MyInt::new(2));
        n.push_front(box MyInt::new(3));
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
        assert_eq!(n.pop_front(), Some(box MyInt::new(0)));
        assert_eq!(n.pop_front(), Some(box MyInt::new(1)));
    }

    #[cfg(test)]
    fn generate_test() -> LinkedList<Box<MyInt>, Links<Box<MyInt>>,
                                     Box<Sentinel<Box<MyInt>, Links<Box<MyInt>>>>> {
        list_from(&[box MyInt::new(0), box MyInt::new(1), box MyInt::new(2),
                    box MyInt::new(3), box MyInt::new(4), box MyInt::new(5),
                    box MyInt::new(6)])
    }

    #[cfg(test)]
    fn list_from<T>(v: &[T]) -> LinkedList<T, Links<T>, Box<Sentinel<T, Links<T>>>>
        where T: Clone + DerefMut + 'static,
              <T as Deref>::Target: Node<T, Links<T>>
    {
        v.iter().cloned().collect()
    }

    #[test]
    fn test_append() {
        // Empty to empty
        {
            let mut m: MyIntList = LinkedList::new();
            let mut n: MyIntList = LinkedList::new();
            m.append(&mut n);
            check_links(&m);
            assert_eq!(m.len(), 0);
            assert_eq!(n.len(), 0);
        }
        // Non-empty to empty
        {
            let mut m: MyIntList = LinkedList::new();
            let mut n: MyIntList = LinkedList::new();
            n.push_back(box MyInt::new(2));
            m.append(&mut n);
            check_links(&m);
            assert_eq!(m.len(), 1);
            assert_eq!(m.pop_back(), Some(box MyInt::new(2)));
            assert_eq!(n.len(), 0);
            check_links(&m);
        }
        // Empty to non-empty
        {
            let mut m: MyIntList = LinkedList::new();
            let mut n: MyIntList = LinkedList::new();
            m.push_back(box MyInt::new(2));
            m.append(&mut n);
            check_links(&m);
            assert_eq!(m.len(), 1);
            assert_eq!(m.pop_back(), Some(box MyInt::new(2)));
            check_links(&m);
        }

        // Non-empty to non-empty
        let v = vec![box MyInt::new(1), box MyInt::new(2), box MyInt::new(3),
                     box MyInt::new(4), box MyInt::new(5)];
        let u = vec![box MyInt::new(9), box MyInt::new(8), box MyInt::new(1),
                     box MyInt::new(2), box MyInt::new(3), box MyInt::new(4),
                     box MyInt::new(5)];
        let mut m = list_from(&v);
        let mut n = list_from(&u);
        m.append(&mut n);
        check_links(&m);
        let mut sum = v;
        sum.push_all(&u);
        assert_eq!(sum.len(), m.len());
        for elt in sum {
            assert_eq!(m.pop_front(), Some(elt))
        }
        assert_eq!(n.len(), 0);
        // let's make sure it's working properly, since we
        // did some direct changes to private members
        n.push_back(box MyInt::new(3));
        assert_eq!(n.len(), 1);
        assert_eq!(n.pop_front(), Some(box MyInt::new(3)));
        check_links(&n);
    }

    // #[test]
    // fn test_split_off() {
    //     // singleton
    //     {
    //         let mut m: MyIntList = LinkedList::new();
    //         m.push_back(box MyInt::new(1));

    //         let p = m.split_off(0);
    //         assert_eq!(m.len(), 0);
    //         assert_eq!(p.len(), 1);
    //         assert_eq!(p.back().unwrap().i, 1);
    //         assert_eq!(p.front().unwrap().i, 1);
    //     }

    //     // not singleton, forwards
    //     {
    //         let u = vec![box MyInt::new(1), box MyInt::new(2),
    //                      box MyInt::new(3), box MyInt::new(4),
    //                      box MyInt::new(5)];
    //         let mut m = list_from(&u);
    //         let mut n = m.split_off(2);
    //         assert_eq!(m.len(), 2);
    //         assert_eq!(n.len(), 3);
    //         for elt in 1..3 {
    //             assert_eq!(m.pop_front(), Some(box MyInt::new(elt)));
    //         }
    //         for elt in 3..6 {
    //             assert_eq!(n.pop_front(), Some(box MyInt::new(elt)));
    //         }
    //     }
    //     // not singleton, backwards
    //     {
    //         let u = vec![box MyInt::new(1), box MyInt::new(2),
    //                      box MyInt::new(3), box MyInt::new(4),
    //                      box MyInt::new(5)];
    //         let mut m = list_from(&u);
    //         let mut n = m.split_off(4);
    //         assert_eq!(m.len(), 4);
    //         assert_eq!(n.len(), 1);
    //         for elt in 1..5 {
    //             assert_eq!(m.pop_front(), Some(box MyInt::new(elt)));
    //         }
    //         for elt in 5..6 {
    //             assert_eq!(n.pop_front(), Some(box MyInt::new(elt)));
    //         }
    //     }

    //     // no-op on the last index
    //     {
    //         let mut m = LinkedList::new();
    //         m.push_back(box MyInt::new(1));

    //         let p = m.split_off(1);
    //         assert_eq!(m.len(), 1);
    //         assert_eq!(p.len(), 0);
    //         assert_eq!(m.back().unwrap().i, 1);
    //         assert_eq!(m.front().unwrap().i, 1);
    //     }
    // }

    #[test]
    fn test_iterator() {
        let m = generate_test();
        for (i, elt) in m.iter().enumerate() {
            assert_eq!(i as i32, elt.i);
        }
        let mut n: MyIntList = LinkedList::new();
        assert_eq!(n.iter().next(), None);
        n.push_front(box MyInt::new(4));
        let mut it = n.iter();
        assert_eq!(it.size_hint(), (1, Some(1)));
        assert_eq!(it.next().unwrap().i, 4);
        assert_eq!(it.size_hint(), (0, Some(0)));
        assert_eq!(it.next(), None);
    }

    #[test]
    fn test_iterator_clone() {
        let mut n: MyIntList = LinkedList::new();
        n.push_back(box MyInt::new(2));
        n.push_back(box MyInt::new(3));
        n.push_back(box MyInt::new(4));
        let mut it = n.iter();
        it.next();
        let mut jt = it.clone();
        assert_eq!(it.next(), jt.next());
        assert_eq!(it.next_back(), jt.next_back());
        assert_eq!(it.next(), jt.next());
    }

    #[test]
    fn test_iterator_double_end() {
        let mut n: MyIntList = LinkedList::new();
        assert_eq!(n.iter().next(), None);
        n.push_front(box MyInt::new(4));
        n.push_front(box MyInt::new(5));
        n.push_front(box MyInt::new(6));
        let mut it = n.iter();
        assert_eq!(it.size_hint(), (3, Some(3)));
        assert_eq!(it.next().unwrap().i, 6);
        assert_eq!(it.size_hint(), (2, Some(2)));
        assert_eq!(it.next_back().unwrap().i, 4);
        assert_eq!(it.size_hint(), (1, Some(1)));
        assert_eq!(it.next_back().unwrap().i, 5);
        assert_eq!(it.next_back(), None);
        assert_eq!(it.next(), None);
    }

    #[test]
    fn test_rev_iter() {
        let m = generate_test();
        for (i, elt) in m.iter().rev().enumerate() {
            assert_eq!((6 - i) as i32, elt.i);
        }
        let mut n: MyIntList = LinkedList::new();
        assert_eq!(n.iter().rev().next(), None);
        n.push_front(box MyInt::new(4));
        let mut it = n.iter().rev();
        assert_eq!(it.size_hint(), (1, Some(1)));
        assert_eq!(it.next().unwrap().i, 4);
        assert_eq!(it.size_hint(), (0, Some(0)));
        assert_eq!(it.next(), None);
    }
}
