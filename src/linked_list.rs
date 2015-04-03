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
use core::cmp::Ordering;
use core::default::Default;
use core::fmt;
use core::hash::{Hasher, Hash};
use core::intrinsics::forget;
use core::iter::{self,FromIterator,IntoIterator};
use core::marker::PhantomData;
use core::mem;
use core::ops::DerefMut;
use core::prelude::*;
use super::rawlink::Rawlink;
#[cfg(any(test,not(feature="nostd")))]
use std::boxed;

///////////////////////
// Trait Definitions //
///////////////////////

/// A trait that allows insertion into a `LinkedList`.

/// The trait is unsafe to implement due to the following constraints:
/// 1) The deref functions must always return the same reference
/// 2) The object cannot be moved while in the `LinkedList`
/// 3) No references (mutable or otherwise) to the target can be used while
///      list operations are ongoing
//  Box and &mut both fulfill these requirements
pub unsafe trait OwningPointer : DerefMut
{
    unsafe fn from_raw(raw: *mut Self::Target) -> Self;

    unsafe fn take(self);
}

/// A trait that allows a struct to be inserted into a `LinkedList`
///
/// Rather than implement this directly, it is expected to use the
/// `define_list_link` macro.
pub unsafe trait Node<L>
    where L: Linkable<Container=Self>
{
    /// Getter for links
    fn get_links(&self) -> &L;

    /// Getter for mutable links
    fn get_links_mut(&mut self) -> &mut L;

    fn get_next(&self) -> &Rawlink<L> {
        self.get_links().get_next()
    }

    fn get_next_mut(&mut self) -> &mut Rawlink<L> {
        self.get_links_mut().get_next_mut()
    }

    fn get_prev(&self) -> &Rawlink<L> {
        self.get_links().get_prev()
    }

    fn get_prev_mut(&mut self) -> &mut Rawlink<L> {
        self.get_links_mut().get_prev_mut()
    }
 }

/// Link trait allowing a struct to be inserted into a `LinkedList`
///
/// The trait is unsafe because any implementation must impl Drop to call
/// check_links()
pub unsafe trait Linkable : Default + Sized
{
    type Container: ?Sized;

    fn get_links(&self) -> &Links<Self>;
    fn get_links_mut(&mut self) -> &mut Links<Self>;
    fn get_next<'a>(&'a self) -> &'a Rawlink<Self>
        where Self: 'a, Self::Container: 'a
    {
        &self.get_links().next
    }
    fn get_next_mut<'a>(&'a mut self) -> &'a mut Rawlink<Self>
        where Self: 'a, Self::Container: 'a
    {
        &mut self.get_links_mut().next
    }
    fn get_prev<'a>(&'a self) -> &'a Rawlink<Self>
        where Self: 'a, Self::Container: 'a
    {
        &self.get_links().prev
    }
    fn get_prev_mut<'a>(&'a mut self) -> &'a mut Rawlink<Self>
        where Self: 'a, Self::Container: 'a
    {
        &mut self.get_links_mut().prev
    }
    fn offset() -> usize;
    unsafe fn container_of(&self) -> &Self::Container {
        let mut val = self as *const _ as usize;
        val -= Self::offset();
        &*(val as *const _)
    }
    unsafe fn container_of_mut(&mut self) -> &mut Self::Container {
        let mut val = self as *const _ as usize;
        val -= Self::offset();
        &mut *(val as *mut _)
    }
    fn check_links(&self) {
        assert!(self.get_next().resolve().is_none());
        assert!(self.get_prev().resolve().is_none());
    }
}

///////////////////////
// Macro Definitions //
///////////////////////

#[macro_export]
macro_rules! offset_of {
    ($container:ty : $field:ident) => (unsafe {
        &(*(0usize as *const $container)).$field as *const _ as usize
    });
}

#[macro_export]
macro_rules! define_list_link {
    ($link:ident = $container:ty : $field:ident) => (
        declare_list_link!($link);
        impl_list_link!($link = $container : $field);
        impl_list_node!($link = $container : $field);
    );
    (pub $link:ident = $container:ty : $field:ident) => (
        declare_list_link!(pub $link);
        impl_list_link!($link = $container : $field);
        impl_list_node!($link = $container : $field);
    );
}

#[macro_export]
macro_rules! declare_list_link {
    ($link:ident) => (
        #[derive(Clone, Default, Debug)]
        struct $link($crate::linked_list::Links<$link>);
    );
    (pub $link:ident) => (
        #[derive(Clone, Default, Debug)]
        pub struct $link($crate::linked_list::Links<$link>);
    );
}

#[macro_export]
macro_rules! impl_list_link {
    ($link:ident = $container:ty : $field:ident) => (
        unsafe impl $crate::linked_list::Linkable for $link {
            type Container = $container;

            #[inline]
            fn get_links(&self) -> &$crate::linked_list::Links<$link> {
                &self.0
            }

            #[inline]
            fn get_links_mut(&mut self) ->
                &mut $crate::linked_list::Links<$link> {
                &mut self.0
            }

            #[inline]
            fn offset() -> usize {
                offset_of!($container : $field)
            }
        }
    );
}

#[macro_export]
macro_rules! impl_list_node {
    ($link:ident = $container:ty : $field:ident) => (
        unsafe impl $crate::linked_list::Node<$link> for $container {
            #[inline]
            fn get_links(&self) -> &$link {
                &self.$field
            }

            #[inline]
            fn get_links_mut(&mut self) -> &mut $link {
                &mut self.$field
            }
        }
    );
}

////////////////////////
// Struct Definitions //
////////////////////////

/// An intrusive doubly-linked list
pub struct LinkedList<T, S, L>
    where T: OwningPointer<Target=S>,
          L: Linkable<Container=T::Target>
{
    length: usize,
    head: Rawlink<L>,
    _marker: PhantomData<T>,
    _marker2: PhantomData<S>
}

#[derive(Clone, Default, Debug)]
pub struct Links<L: Linkable>
{
    prev: Rawlink<L>,
    next: Rawlink<L>
}

/// An iterator over references to the items of a `LinkedList`
pub struct Iter<T, L: Linkable<Container=T>> {
    head: Rawlink<L>,
    tail: Rawlink<L>,
    nelem: usize,
}

/// An iterator over mutable references to the items of a `LinkedList`
pub struct IterMut<'a, T, S, L>
    where T: OwningPointer<Target=S> + 'a,
          S: Node<L> + 'a,
          L: Linkable<Container=T::Target> + 'a
{
    list: &'a mut LinkedList<T, S, L>,
    head: Rawlink<L>,
    tail: Rawlink<L>,
    nelem: usize,
}

pub struct IntoIter<T, S, L>
    where T: OwningPointer<Target=S>,
          L: Linkable<Container=T::Target>
{
    list: LinkedList<T, S, L>
}

// LinkedList impls

impl<T, S, L> LinkedList<T, S, L>
    where T: OwningPointer<Target=S>,
          S: Node<L>,
          L: Linkable<Container=T::Target>
{
    #[inline]
    pub fn new() -> LinkedList<T, S, L> {
        LinkedList { length: 0, head: Rawlink::none(),
                     _marker: PhantomData, _marker2: PhantomData }
    }

    /// Moves all elements from `other` to the end of the list.
    ///
    /// This reuses all the nodes from `other` and moves them into `self`. After
    /// this operation, `other` becomes empty.
    ///
    /// This operation should compute in O(1) time and O(1) memory.
    ///
    /// # Examples
    ///
    /// ```
    /// # #[macro_use] extern crate intrusive_containers;
    /// use std::default::Default;
    /// use intrusive_containers::LinkedList;
    ///
    /// struct MyInt {
    ///     links: MyLink,
    ///     i: i32,
    /// }
    ///
    /// define_list_link!(MyLink = MyInt : links);
    ///
    /// impl MyInt {
    ///   pub fn new(i: i32) -> MyInt {
    ///     MyInt { links: Default::default(), i: i}
    ///   }
    /// }
    ///
    /// # fn main() {
    /// let mut a = LinkedList::new();
    /// let mut b = LinkedList::new();
    /// a.push_back(Box::new(MyInt::new(1)));
    /// a.push_back(Box::new(MyInt::new(2)));
    /// b.push_back(Box::new(MyInt::new(3)));
    /// b.push_back(Box::new(MyInt::new(4)));
    ///
    /// a.append(&mut b);
    ///
    /// for e in a.iter() {
    ///     println!("{}", e.i); // prints 1, then 2, then 3, then 4
    /// }
    /// println!("{}", b.len()); // prints 0
    /// # }
    /// ```
    pub fn append(&mut self, other: &mut LinkedList<T, S, L>) {
        match self.head.resolve_mut() {
            None => {
                self.length = other.length;
                self.head = other.head.take();
            },
            Some(head) => {
                let tail = head.get_prev_mut().resolve_mut().unwrap();
                match other.head.take().resolve_mut() {
                    None => return,
                    Some(other_head) => {
                        let other_tail =
                            other_head.get_prev_mut().resolve_mut().unwrap();
                        *other_tail.get_next_mut() = Rawlink::some(head);
                        *other_head.get_prev_mut() = Rawlink::some(tail);
                        *tail.get_next_mut() = Rawlink::some(other_head);
                        *head.get_prev_mut() = Rawlink::some(other_tail);
                        self.length += other.length;
                    }
                }
            }
        }
        other.length = 0;
    }


    /// Provides a forward iterator.
    #[inline]
    pub fn iter(&self) -> Iter<S, L> {
        let tail = if self.length == 0 {
            Rawlink::none()
        } else {
            *self.head.resolve().unwrap().get_prev()
        };
        Iter{nelem: self.length, head: self.head,
             tail: tail}
    }

    /// Consumes the list into an iterator yielding elements by value.
    #[inline]
    pub fn into_iter(self) -> IntoIter<T, S, L> {
        IntoIter{list: self}
    }

    /// Returns `true` if the `LinkedList` is empty
    ///
    /// This operation should compute in O(1) time
    ///
    /// # Examples
    ///
    /// ```
    /// # #[macro_use] extern crate intrusive_containers;
    /// use std::default::Default;
    /// use intrusive_containers::LinkedList;
    ///
    /// struct MyInt {
    ///     links: MyLink,
    ///     i: i32,
    /// }
    ///
    /// define_list_link!(MyLink = MyInt : links);
    ///
    /// impl MyInt {
    ///   pub fn new(i: i32) -> MyInt {
    ///     MyInt { links: Default::default(), i: i}
    ///   }
    /// }
    ///
    /// # fn main() {
    /// let mut dl = LinkedList::new();
    /// assert!(dl.is_empty());
    ///
    /// dl.push_front(Box::new(MyInt::new(1)));
    /// assert!(!dl.is_empty());
    /// # }
    /// ```
    #[inline]
    pub fn is_empty(&self) -> bool {
        self.length == 0
    }

    /// Returns the length of the `LinkedList`.
    ///
    /// This operation should compute in O(1) time.
    ///
    /// # Examples
    ///
    /// ```
    /// # #[macro_use] extern crate intrusive_containers;
    /// use std::default::Default;
    /// use intrusive_containers::LinkedList;
    ///
    /// struct MyInt {
    ///     links: MyLink,
    ///     i: i32,
    /// }
    ///
    /// define_list_link!(MyLink = MyInt : links);
    ///
    /// impl MyInt {
    ///   pub fn new(i: i32) -> MyInt {
    ///     MyInt { links: Default::default(), i: i}
    ///   }
    /// }
    ///
    /// # fn main() {
    /// let mut dl = LinkedList::new();
    ///
    /// dl.push_front(Box::new(MyInt::new(2)));
    /// assert_eq!(dl.len(), 1);
    ///
    /// dl.push_front(Box::new(MyInt::new(1)));
    /// assert_eq!(dl.len(), 2);
    ///
    /// dl.push_back(Box::new(MyInt::new(3)));
    /// assert_eq!(dl.len(), 3);
    /// # }
    /// ```
    #[inline]
    pub fn len(&self) -> usize {
        self.length
    }

    /// Provides a reference to the front element, or `None` if the list is
    /// empty.
    #[inline]
    pub fn front(&self) -> Option<&T::Target> {
        self.head.resolve().map(|head| unsafe {head.container_of()})
    }

    /// Provides a mutable reference to the front element, or `None` if the list
    /// is empty.
    #[inline]
    pub fn front_mut(&mut self) -> Option<&mut T::Target> {
        self.head.resolve_mut().map(|head| unsafe {head.container_of_mut()})
    }

    /// Provides a reference to the back element, or `None` if the list is
    /// empty.
    #[inline]
    pub fn back(&self) -> Option<&T::Target> {
        self.head.resolve().map(|head| {
            unsafe{head.get_prev().resolve().unwrap().container_of()}
        })
    }

    /// Provides a mutable reference to the back element, or `None` if the list
    /// is empty.
    #[inline]
    pub fn back_mut(&mut self) -> Option<&mut T::Target> {
        // if pprev is not none, then it points to the link before the tail
        self.head.resolve_mut().map(|head| {
            unsafe {
                head.get_prev_mut().resolve_mut().unwrap().container_of_mut()
            }
        })
    }

    fn insert(&mut self, elt: &mut L, prev: &mut L, next: &mut L) {
        *next.get_prev_mut() = Rawlink::some(elt);
        *elt.get_next_mut() = Rawlink::some(next);
        *elt.get_prev_mut() = Rawlink::some(prev);
        *prev.get_next_mut() = Rawlink::some(elt);
        self.length += 1;
    }

    fn delete(&mut self, elt: &mut L) {
        debug_assert!(*elt.get_next() != Rawlink::none());
        debug_assert!(*elt.get_prev() != Rawlink::none());

        let next = elt.get_next_mut().resolve_mut().unwrap();
        let prev = elt.get_prev_mut().resolve_mut().unwrap();
        *next.get_prev_mut() = Rawlink::some(prev);
        *prev.get_next_mut() = Rawlink::some(next);

        elt.get_next_mut().take();
        elt.get_prev_mut().take();
        self.length -= 1;
    }

    /// Adds an element first in the list.
    ///
    /// This operation should compute in O(1) time.
    pub fn push_front(&mut self, mut elt: T) {
        // ensure links are not already being used
        elt.get_links().check_links();

        if self.is_empty() {
            *elt.get_next_mut() = Rawlink::some(elt.get_links_mut());
            *elt.get_prev_mut() = Rawlink::some(elt.get_links_mut());
            self.length += 1;
        } else {
            let head = self.head.resolve_mut().unwrap();
            let tail = head.get_prev_mut().resolve_mut().unwrap();
            self.insert(elt.get_links_mut(), tail, head);
        }
        self.head = Rawlink::some(elt.get_links_mut());
        unsafe { elt.take() };
    }

    /// Removes the first element and returns it, or `None` if the list is
    /// empty.
    ///
    /// This operation should compute in O(1) time.
    pub fn pop_front(&mut self) -> Option<T> {
        self.head.take().resolve_mut().map(|mut head| {
            if self.length == 1 {
                self.head = Rawlink::none();
            } else {
                self.head = *head.get_next_mut();
            }
            self.delete(head);
            unsafe {
                T::from_raw(head.container_of_mut() as *mut _)
            }
        })
    }

    /// Appends an element to the back of a list
    ///
    /// This operation should compute in O(1) time.
    pub fn push_back(&mut self, mut elt: T) {
        if self.is_empty() {
            return self.push_front(elt);
        }

        // ensure links are not already being used
        elt.get_links().check_links();

        let head = self.head.resolve_mut().unwrap();
        let tail = head.get_prev_mut().resolve_mut().unwrap();
        self.insert(elt.get_links_mut(), tail, head);
        unsafe { elt.take() };
    }

    /// Removes the last element from a list and returns it, or `None` if
    /// it is empty.
    ///
    /// This operation should compute in O(1) time
    pub fn pop_back(&mut self) -> Option<T> {
        if self.len() <= 1 { return self.pop_front(); }

        let head = self.head.resolve_mut().unwrap();
        let tail = head.get_prev_mut().resolve_mut().unwrap();
        self.delete(tail);
        Some(unsafe {T::from_raw(tail.container_of_mut() as *mut _)})
    }

    /// Splits the list into two at the given index. Returns everything after the given index,
    /// including the index.
    ///
    /// # Panics
    ///
    /// Panics if `at > len`.
    ///
    /// This operation should compute in O(n) time.
    pub fn split_off(&mut self, at: usize) -> LinkedList<T, S, L> {
        let len = self.len();
        assert!(at <= len, "Cannot split off at a nonexistent index");
        if at == 0 {
            return mem::replace(self, LinkedList::new());
        } else if at == len {
            return LinkedList::new();
        }

        // Below, we iterate towards the `i-1`th node, either from the start or the end,
        // depending on which would be faster.
        let mut split_node = if at - 1 <= len - 1 - (at - 1) {
            let mut iter = unsafe { self.iter_mut() };
            // instead of skipping using .skip() (which creates a new struct),
            // we skip manually so we can access the head field without
            // depending on implementation details of Skip
            for _ in 0..at - 1 {
                iter.next();
            }
            iter.head
        } else {
            // better off starting from the end
            let mut iter = unsafe { self.iter_mut() };
            for _ in 0..len - 1 - (at - 1) {
                iter.next_back();
            }
            iter.tail
        };

        let mut pre_split = split_node.resolve_mut().unwrap();
        let mut post_split = pre_split.get_next_mut().resolve_mut().unwrap();
        let mut head = self.head.resolve_mut().unwrap();
        let mut tail = head.get_prev_mut().resolve_mut().unwrap();

        *head.get_prev_mut() = Rawlink::some(pre_split);
        *pre_split.get_next_mut() = Rawlink::some(head);
        *post_split.get_prev_mut() = Rawlink::some(tail);
        *tail.get_next_mut() = Rawlink::some(post_split);

        self.length = at;
        LinkedList {
            head: Rawlink::some(post_split),
            length: len - at,
            _marker: PhantomData,
            _marker2: PhantomData
        }
    }

}

impl<'a, T, S, L> LinkedList<T, S, L>
    where T: OwningPointer<Target=S> + 'a,
          S: Node<L> + 'a,
          L: Linkable<Container=T::Target> + 'a
{
    /// Provides a forward iterator with mutable references
    ///
    /// This operation is marked unsafe because it would be possible to use
    /// `mem::replace` which would invalidate the links
    #[inline]
    pub unsafe fn iter_mut(&'a mut self) -> IterMut<'a, T, S, L> {
        let tail = if self.length == 0 {
            Rawlink::none()
        } else {
            *self.head.resolve().unwrap().get_prev()
        };
        IterMut {
            nelem: self.length,
            head: self.head,
            tail: tail,
            list: self
        }
    }
}

impl<T, S, L> Default for LinkedList<T, S, L>
    where T: OwningPointer<Target=S>,
          S: Node<L>,
          L: Linkable<Container=T::Target>
{
    #[inline]
    fn default() -> LinkedList<T, S, L> {
        LinkedList::new()
    }
}

impl<T, S, L> Clone for LinkedList<T, S, L>
    where T: OwningPointer<Target=S> + Clone,
          S: Node<L>,
          L: Linkable<Container=T::Target>
{
    fn clone(&self) -> LinkedList<T, S, L> {
        self.iter().map(|x| {
            unsafe {
                let t = T::from_raw(x as *const S as *mut S);
                let ret = t.clone();
                t.take();
                ret
            }
        }).collect()
    }
}

impl<T, S, L> fmt::Debug for LinkedList<T, S, L>
    where T: OwningPointer<Target=S>,
          S: Node<L> + fmt::Debug,
          L: Linkable<Container=T::Target>
{
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        try!(write!(f, "["));

        for (i, e) in self.iter().enumerate() {
            if i != 0 { try!(write!(f, ", ")); }
            try!(write!(f, "{:?}", e));
        }

        write!(f, "]")
    }
}

impl<T, S, L> Drop for LinkedList<T, S, L>
    where T: OwningPointer<Target=S>,
          L: Linkable<Container=T::Target>
{
    fn drop(&mut self) {
        loop {
            if self.length == 0 {
                return;
            }
            let head = self.head.resolve_mut().unwrap();
            self.head = *head.get_next();
            head.get_next_mut().take();
            head.get_prev_mut().take();
            self.length -= 1;
        }
    }
}

impl<T, S, L> Hash for LinkedList<T, S, L>
    where T: OwningPointer<Target=S>,
          S: Node<L> + Hash,
          L: Linkable<Container=T::Target>
{
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.len().hash(state);
        for elt in self {
            elt.hash(state);
        }
    }
}

impl<T, S, L> Extend<T> for LinkedList<T, S, L>
    where T: OwningPointer<Target=S>,
          S: Node<L>,
          L: Linkable<Container=T::Target>
{
    fn extend<I: IntoIterator<Item=T>>(&mut self, iter: I) {
        for elt in iter { self.push_back(elt); }
    }
}

impl<T, S, L> FromIterator<T> for LinkedList<T, S, L>
    where T: OwningPointer<Target=S>,
          S: Node<L>,
          L: Linkable<Container=T::Target>
{
    fn from_iter<I: IntoIterator<Item=T>>(iter: I) -> LinkedList<T, S, L> {
        let mut ret = LinkedList::new();
        ret.extend(iter);
        ret
    }
}

impl<T, S, L> IntoIterator for LinkedList<T, S, L>
    where T: OwningPointer<Target=S>,
          S: Node<L>,
          L: Linkable<Container=T::Target>
{
    type Item = T;
    type IntoIter = IntoIter<T, S, L>;

    fn into_iter(self) -> IntoIter<T, S, L> {
        self.into_iter()
    }
}

impl<'a, T, S, L> IntoIterator for &'a LinkedList<T, S, L>
    where T: OwningPointer<Target=S>,
          S: Node<L> + 'a,
          L: Linkable<Container=T::Target> + 'a
{
    type Item = &'a S;
    type IntoIter = Iter<S, L>;

    fn into_iter(self) -> Iter<S, L> {
        self.iter()
    }
}

// impl<'a, T, S, L> IntoIterator for &'a mut LinkedList<T, S, L>
//     where T: OwningPointer<Target=S> + 'a,
//           S: Node<L> + 'a,
//           L: Linkable<Container=T::Target> + 'a
// {
//     type Item = &'a mut S;
//     type IntoIter = IterMut<'a, T, S, L>;

//     fn into_iter(self) -> IterMut<'a, T, S, L> {
//         self.iter_mut()
//     }
// }

impl<T, S, L> PartialEq for LinkedList<T, S, L>
    where T: OwningPointer<Target=S>,
          S: Node<L> + PartialEq,
          L: Linkable<Container=T::Target>
{
    fn eq(&self, other: &LinkedList<T, S, L>) -> bool {
        self.len() == other.len() &&
            iter::order::eq(self.iter(), other.iter())
    }

    fn ne(&self, other: &LinkedList<T, S, L>) -> bool {
        self.len() != other.len() ||
            iter::order::ne(self.iter(), other.iter())
    }
}

impl<T, S, L> Eq for LinkedList<T, S, L>
    where T: OwningPointer<Target=S>,
          S: Node<L> + Eq,
          L: Linkable<Container=T::Target>
{}

impl<T, S, L> PartialOrd for LinkedList<T, S, L>
    where T: OwningPointer<Target=S>,
          S: Node<L> + PartialOrd,
          L: Linkable<Container=T::Target>
{
    fn partial_cmp(&self, other: &LinkedList<T, S, L>) -> Option<Ordering> {
        iter::order::partial_cmp(self.iter(), other.iter())
    }
}

impl<T, S, L> Ord for LinkedList<T, S, L>
    where T: OwningPointer<Target=S>,
          S: Node<L> + Ord,
          L: Linkable<Container=T::Target>
{
    fn cmp(&self, other: &LinkedList<T, S, L>) -> Ordering {
        iter::order::cmp(self.iter(), other.iter())
    }
}

// Links impls

impl<L: Linkable> Drop for Links<L>
{
    fn drop(&mut self) {
        assert!(self.next.resolve().is_none());
        assert!(self.prev.resolve().is_none());
    }
}

// Iter impls

impl<T, L: Linkable<Container=T>> Clone for Iter<T, L> {
    fn clone(&self) -> Iter<T, L> {
        Iter {
            head: self.head,
            tail: self.tail,
            nelem: self.nelem,
        }
    }
}

impl<'a, T: 'a, L: Linkable<Container=T> + 'a> Iterator for Iter<T, L>
{
    type Item = &'a T;

    #[inline]
    fn next(&mut self) -> Option<&'a T> {
        if self.nelem == 0 {
            return None;
        }
        let head = self.head.resolve().unwrap();
        self.nelem -= 1;
        self.head = *head.get_next();
        let ret = unsafe { head.container_of() };
        Some(ret)
    }

    #[inline]
    fn size_hint(&self) -> (usize, Option<usize>) {
        (self.nelem, Some(self.nelem))
    }
}

impl<'a, T: 'a, L: Linkable<Container=T> + 'a> DoubleEndedIterator
    for Iter<T, L>
{
    #[inline]
    fn next_back(&mut self) -> Option<&'a T> {
        if self.nelem == 0 {
            return None;
        }
        let tail = self.tail.resolve().unwrap();
        self.nelem -= 1;
        self.tail = *tail.get_prev();
        let ret = unsafe { tail.container_of() };
        Some(ret)
    }
}

impl<'a, T: 'a, L: Linkable<Container=T> + 'a> ExactSizeIterator
    for Iter<T, L> {}

// // IterMut impls

impl<'a, T, S, L> Iterator for IterMut<'a, T, S, L>
    where T: OwningPointer<Target=S> + 'a,
          S: Node<L> + 'a,
          L: Linkable<Container=T::Target> + 'a
{
    type Item = &'a mut S;

    #[inline]
    fn next(&mut self) -> Option<&'a mut S> {
        if self.nelem == 0 {
            return None;
        }
        let head = self.head.resolve_mut().unwrap();
        self.nelem -= 1;
        self.head = *head.get_next();
        let ret = unsafe { head.container_of_mut() };
        Some(ret)
    }

    #[inline]
    fn size_hint(&self) -> (usize, Option<usize>) {
        (self.nelem, Some(self.nelem))
    }
}

impl<'a, T, S, L> DoubleEndedIterator for IterMut<'a, T, S, L>
    where T: OwningPointer<Target=S> + 'a,
          S: Node<L> + 'a,
          L: Linkable<Container=T::Target> + 'a
{
    #[inline]
    fn next_back(&mut self) -> Option<&'a mut S> {
        if self.nelem == 0 {
            return None;
        }
        let tail = self.tail.resolve_mut().unwrap();
        self.nelem -= 1;
        self.tail = *tail.get_prev();
        let ret = unsafe { tail.container_of_mut() };
        Some(ret)
    }
}

impl<'a, T, S, L> ExactSizeIterator for IterMut<'a, T, S, L>
    where T: OwningPointer<Target=S> + 'a,
          S: Node<L> + 'a,
          L: Linkable<Container=T::Target> + 'a
{}

impl<'a, T, S, L> IterMut<'a, T, S, L>
    where T: OwningPointer<Target=S> + 'a,
          S: Node<L> + 'a,
          L: Linkable<Container=T::Target> + 'a
{
    /// Inserts `elt` just after the element most recently returned by `.next()`.
    /// The inserted element does not appear in the iteration.
    #[inline]
    pub fn insert_next(&mut self, mut elt: T) {
        // ensure links are not already being used
        elt.get_links().check_links();

        if self.nelem == 0 {
            return self.list.push_back(elt);
        }

        if self.head == self.list.head {
            return self.list.push_front(elt);
        }

        let next = self.head.resolve_mut().unwrap();
        let prev = next.get_prev_mut().resolve_mut().unwrap();
        self.list.insert(elt.get_links_mut(), prev, next);
        unsafe { elt.take() };
    }

    /// Provides a reference to the next element, without changing the iterator.
    #[inline]
    pub fn peek_next(&mut self) -> Option<&'a mut S> {
        if self.nelem == 0 {
            return None
        }
        let head = self.head.resolve_mut().unwrap();
        let ret = unsafe { head.container_of_mut() };
        Some(ret)
    }
}

// IntoIter impls

impl<T, S, L> Iterator for IntoIter<T, S, L>
    where T: OwningPointer<Target=S>,
          S: Node<L>,
          L: Linkable<Container=T::Target>
{
    type Item = T;

    #[inline]
    fn next(&mut self) -> Option<T> { self.list.pop_front() }

    #[inline]
    fn size_hint(&self) -> (usize, Option<usize>) {
        (self.list.length, Some(self.list.length))
    }
}

impl<T, S, L> DoubleEndedIterator for IntoIter<T, S, L>
    where T: OwningPointer<Target=S>,
          S: Node<L>,
          L: Linkable<Container=T::Target>
{
    #[inline]
    fn next_back(&mut self) -> Option<T> { self.list.pop_back() }
}

impl<T, S, L> Clone for IntoIter<T, S, L>
    where T: OwningPointer<Target=S> + Clone,
          S: Node<L>,
          L: Linkable<Container=T::Target>
{
    #[inline]
    fn clone(&self) -> IntoIter<T, S, L> {
        IntoIter { list: self.list.clone() }
    }
}

// OwningPointer impls

unsafe impl<'a, T> OwningPointer for &'a mut T {
    #[inline]
    unsafe fn from_raw(raw: *mut T) -> &'a mut T {
        &mut *raw
    }

    #[inline]
    unsafe fn take(self) {
        forget(self);
    }
}

#[cfg(any(test,not(feature="nostd")))]
unsafe impl<T> OwningPointer for Box<T> {
    #[inline]
    unsafe fn from_raw(raw: *mut T) -> Box<T> {
        Box::from_raw(raw)
    }

    #[inline]
    unsafe fn take(self) {
        boxed::into_raw(self);
    }
}

///////////
// Tests //
///////////

#[cfg(test)]
mod tests {
    use std::prelude::v1::*;
    use std::cmp::Ordering;
    use std::default::Default;
    use std::hash::{self, Hash, Hasher, SipHasher};
    use std::fmt;
    use std::thread;
    use super::{LinkedList, OwningPointer, Node, Linkable};
    use rand;

    struct MyInt {
        links: MyLink,
        i: i32,
    }

    define_list_link!(MyLink = MyInt : links);

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

    impl Hash for MyInt {
        fn hash<H: Hasher>(&self, state: &mut H) {
            self.i.hash(state);
        }
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

    impl Eq for MyInt {}

    impl PartialOrd for MyInt {
        fn partial_cmp(&self, other: &MyInt) -> Option<Ordering> {
            self.i.partial_cmp(&other.i)
        }
    }

    impl Ord for MyInt {
        fn cmp(&self, other: &MyInt) -> Ordering {
            self.i.cmp(&other.i)
        }
    }

    impl MyInt {
        pub fn new(i: i32) -> MyInt {
            MyInt { links: Default::default(), i: i}
        }
    }

    pub fn check_links<T, S, L>(list: &LinkedList<T, S, L>)
        where T: OwningPointer<Target=S>,
              S: Node<L>,
              L: Linkable<Container=S> + fmt::Debug,
    {
        let mut len = 0;
        let mut head: &L;
        let mut prev_links: &L;
        let mut link_ptr: &L;
        match list.head.resolve() {
            None => { assert_eq!(0, list.length); return }
            Some(ref links) => {
                head = links;
                link_ptr = links;
                prev_links = links.get_prev().resolve().unwrap();
            }
        }

        loop {
            match link_ptr.get_prev().resolve() {
                None => panic!("unset prev link"),
                Some(prev) => {
                    assert_eq!(prev_links as *const L, prev as *const L);
                }
            }
            match link_ptr.get_next().resolve() {
                None => panic!("unset next link"),
                Some(next) => {
                    len += 1;
                    if next as *const L == head as *const L {
                        break;
                    }
                    prev_links = link_ptr;
                    link_ptr = next;
                }
            }
        }
        assert_eq!(len, list.length);
    }

    #[test]
    fn test_basic() {
        let mut m = LinkedList::new();

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


        let mut n = LinkedList::new();
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

    #[test]
    fn test_mut_ref() {
        let mut m = MyInt::new(0);
        let mut n = LinkedList::new();
        n.push_front(&mut m);
    }

    #[cfg(test)]
    fn generate_test() -> LinkedList<Box<MyInt>, MyInt, MyLink> {
        list_from(&[box MyInt::new(0), box MyInt::new(1), box MyInt::new(2),
                    box MyInt::new(3), box MyInt::new(4), box MyInt::new(5),
                    box MyInt::new(6)])
    }

    #[cfg(test)]
    fn list_from<T, S, L>(v: &[T]) -> LinkedList<T, S, L>
        where T: OwningPointer<Target=S> + Clone,
              S: Node<L>,
              L: Linkable<Container=S>
    {
        v.iter().cloned().collect()
    }

    #[test]
    fn test_append() {
        // Empty to empty
        {
            let mut m: LinkedList<Box<MyInt>, MyInt, MyLink> =
                LinkedList::new();
            let mut n: LinkedList<Box<MyInt>, MyInt, MyLink> =
                LinkedList::new();
            m.append(&mut n);
            check_links(&m);
            assert_eq!(m.len(), 0);
            assert_eq!(n.len(), 0);
        }
        // Non-empty to empty
        {
            let mut m = LinkedList::new();
            let mut n = LinkedList::new();
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
            let mut m = LinkedList::new();
            let mut n = LinkedList::new();
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

    #[test]
    fn test_split_off() {
        // singleton
        {
            let mut m = LinkedList::new();
            m.push_back(box MyInt::new(1));

            let p = m.split_off(0);
            assert_eq!(m.len(), 0);
            assert_eq!(p.len(), 1);
            assert_eq!(p.back().unwrap().i, 1);
            assert_eq!(p.front().unwrap().i, 1);
        }

        // not singleton, forwards
        {
            let u = vec![box MyInt::new(1), box MyInt::new(2),
                         box MyInt::new(3), box MyInt::new(4),
                         box MyInt::new(5)];
            let mut m = list_from(&u);
            let mut n = m.split_off(2);
            assert_eq!(m.len(), 2);
            assert_eq!(n.len(), 3);
            for elt in 1..3 {
                assert_eq!(m.pop_front(), Some(box MyInt::new(elt)));
            }
            for elt in 3..6 {
                assert_eq!(n.pop_front(), Some(box MyInt::new(elt)));
            }
        }
        // not singleton, backwards
        {
            let u = vec![box MyInt::new(1), box MyInt::new(2),
                         box MyInt::new(3), box MyInt::new(4),
                         box MyInt::new(5)];
            let mut m = list_from(&u);
            let mut n = m.split_off(4);
            assert_eq!(m.len(), 4);
            assert_eq!(n.len(), 1);
            for elt in 1..5 {
                assert_eq!(m.pop_front(), Some(box MyInt::new(elt)));
            }
            for elt in 5..6 {
                assert_eq!(n.pop_front(), Some(box MyInt::new(elt)));
            }
        }

        // no-op on the last index
        {
            let mut m = LinkedList::new();
            m.push_back(box MyInt::new(1));

            let p = m.split_off(1);
            assert_eq!(m.len(), 1);
            assert_eq!(p.len(), 0);
            assert_eq!(m.back().unwrap().i, 1);
            assert_eq!(m.front().unwrap().i, 1);
        }
    }

    #[test]
    fn test_iterator() {
        let m = generate_test();
        for (i, elt) in m.iter().enumerate() {
            assert_eq!(i as i32, elt.i);
        }
        let mut n = LinkedList::new();
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
        let mut n = LinkedList::new();
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
        let mut n = LinkedList::new();
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
        let mut n = LinkedList::new();
        assert_eq!(n.iter().rev().next(), None);
        n.push_front(box MyInt::new(4));
        let mut it = n.iter().rev();
        assert_eq!(it.size_hint(), (1, Some(1)));
        assert_eq!(it.next().unwrap().i, 4);
        assert_eq!(it.size_hint(), (0, Some(0)));
        assert_eq!(it.next(), None);
    }

    #[test]
    fn test_mut_iter() {
        let mut m = generate_test();
        let mut len = m.len();
        for (i, elt) in unsafe {m.iter_mut()}.enumerate() {
            assert_eq!(i as i32, elt.i);
            len -= 1;
        }
        assert_eq!(len, 0);
        let mut n = LinkedList::new();
        assert!(unsafe{n.iter_mut()}.next().is_none());
        n.push_front(box MyInt::new(4));
        n.push_back(box MyInt::new(5));
        let mut it = unsafe { n.iter_mut() };
        assert_eq!(it.size_hint(), (2, Some(2)));
        assert!(it.next().is_some());
        assert!(it.next().is_some());
        assert_eq!(it.size_hint(), (0, Some(0)));
        assert!(it.next().is_none());
    }

    #[test]
    fn test_iterator_mut_double_end() {
        let mut n = LinkedList::new();
        assert!(unsafe{n.iter_mut()}.next_back().is_none());
        n.push_front(box MyInt::new(4));
        n.push_front(box MyInt::new(5));
        n.push_front(box MyInt::new(6));
        let mut it = unsafe{n.iter_mut()};
        assert_eq!(it.size_hint(), (3, Some(3)));
        assert_eq!(it.next().unwrap().i, 6);
        assert_eq!(it.size_hint(), (2, Some(2)));
        assert_eq!(it.next_back().unwrap().i, 4);
        assert_eq!(it.size_hint(), (1, Some(1)));
        assert_eq!(it.next_back().unwrap().i, 5);
        assert!(it.next_back().is_none());
        assert!(it.next().is_none());
    }

    #[test]
    fn test_insert_next() {
        let mut m = list_from(&[box MyInt::new(0), box MyInt::new(2),
                                box MyInt::new(4),box MyInt::new(6),
                                box MyInt::new(8)]);
        let len = m.len();
        {
            let mut it = unsafe {m.iter_mut()};
            it.insert_next(box MyInt::new(-2));
            loop {
                match it.next() {
                    None => break,
                    Some(elt) => {
                        it.insert_next(box MyInt::new(elt.i + 1));
                        match it.peek_next() {
                            Some(x) => assert_eq!(x.i, elt.i + 2),
                            None => assert_eq!(8, elt.i),
                        }
                    }
                }
            }
            it.insert_next(box MyInt::new(0));
            it.insert_next(box MyInt::new(1));
        }
        check_links(&m);
        assert_eq!(m.len(), 3 + len * 2);
        assert_eq!(m.into_iter().collect::<Vec<_>>(), [box MyInt::new(-2),
                                                       box MyInt::new(0),
                                                       box MyInt::new(1),
                                                       box MyInt::new(2),
                                                       box MyInt::new(3),
                                                       box MyInt::new(4),
                                                       box MyInt::new(5),
                                                       box MyInt::new(6),
                                                       box MyInt::new(7),
                                                       box MyInt::new(8),
                                                       box MyInt::new(9),
                                                       box MyInt::new(0),
                                                       box MyInt::new(1)]);
    }

    #[test]
    fn test_mut_rev_iter() {
        let mut m = generate_test();
        for (i, elt) in unsafe{m.iter_mut()}.rev().enumerate() {
            assert_eq!((6 - i) as i32, elt.i);
        }
        let mut n = LinkedList::new();
        assert!(unsafe{n.iter_mut()}.rev().next().is_none());
        n.push_front(box MyInt::new(4));
        let mut it = unsafe{n.iter_mut()}.rev();
        assert!(it.next().is_some());
        assert!(it.next().is_none());
    }

    #[test]
    fn test_send() {
        let n = list_from(&[box MyInt::new(1), box MyInt::new(2),
                            box MyInt::new(3)]);
        thread::spawn(move || {
            check_links(&n);
            let a = list_from(&[box MyInt::new(1),box MyInt::new(2),
                                box MyInt::new(3)]);
            assert_eq!(a, n);
        }).join().ok().unwrap();
    }

    #[test]
    fn test_eq() {
        let mut n = list_from(&[]);
        let mut m = list_from(&[]);
        assert!(n == m);
        n.push_front(box MyInt::new(1));
        assert!(n != m);
        m.push_back(box MyInt::new(1));
        assert!(n == m);

        let n = list_from(&[box MyInt::new(2), box MyInt::new(3),
                            box MyInt::new(4)]);
        let m = list_from(&[box MyInt::new(1),
                            box MyInt::new(2),
                            box MyInt::new(3)]);
        assert!(n != m);
    }

    #[test]
    fn test_hash() {
      let mut x = LinkedList::new();
      let mut y = LinkedList::new();

      assert!(hash::hash::<_, SipHasher>(&x) == hash::hash::<_, SipHasher>(&y));

      x.push_back(box MyInt::new(1));
      x.push_back(box MyInt::new(2));
      x.push_back(box MyInt::new(3));

      y.push_front(box MyInt::new(3));
      y.push_front(box MyInt::new(2));
      y.push_front(box MyInt::new(1));

      assert!(hash::hash::<_, SipHasher>(&x) == hash::hash::<_, SipHasher>(&y));
    }

    #[test]
    fn test_ord() {
        let n = list_from(&[]);
        let m = list_from(&[box MyInt::new(1),
                            box MyInt::new(2),
                            box MyInt::new(3)]);
        assert!(n < m);
        assert!(m > n);
        assert!(n <= n);
        assert!(n >= n);
    }

    #[test]
    fn test_fuzz() {
        for _ in 0..25 {
            fuzz_test(3);
            fuzz_test(16);
            fuzz_test(189);
        }
    }

    #[cfg(test)]
    fn fuzz_test(sz: i32) {
        let mut m = LinkedList::new();
        let mut v = vec![];
        for i in 0..sz {
            check_links(&m);
            let r: u8 = rand::random();
            match r % 6 {
                0 => {
                    m.pop_back();
                    v.pop();
                }
                1 => {
                    if !v.is_empty() {
                        m.pop_front();
                        v.remove(0);
                    }
                }
                2 | 4 =>  {
                    m.push_front(box MyInt::new(-i));
                    v.insert(0, box MyInt::new(-i));
                }
                3 | 5 | _ => {
                    m.push_back(box MyInt::new(i));
                    v.push(box MyInt::new(i));
                }
            }
        }

        check_links(&m);

        let mut i = 0;
        for (ref a, ref b) in m.into_iter().zip(v.iter()) {
            i += 1;
            assert_eq!(a.i, b.i);
        }
        assert_eq!(i, v.len());
    }
}
