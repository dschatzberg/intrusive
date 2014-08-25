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
//! The 'DList' allows elements to be inserted or removed from either end.
//! Due to the nature of intrusive data structures, some methods are deemed
//! unsafe
use core::kinds::marker::ContravariantLifetime;
use core::default::Default;
use core::fmt;
use core::iter;
use core::mem;
use core::prelude::*;
use core::ptr;

use {Deque, Mutable, MutableSeq};

/// An intrusive doubly-linked list.
pub struct DList<T> {
    head: RawLink<T>,
}

struct RawLink<T> {
    ptr: *mut T
}

/// Links allowing a struct to be inserted into a `Dlist`
pub struct Links<T> {
    next: RawLink<T>,
    prev: RawLink<T>
}

#[unsafe_destructor]
impl<T: Node<T>> Drop for Links<T> {
    #[inline]
    fn drop(&mut self) {
        assert!(self.next.is_none());
        assert!(self.prev.is_none());
    }
}

/// A trait that allows a struct to be inserted into a `Dlist`
pub trait Node<T> {
    /// Getter for links
    fn list_hook<'a>(&'a self) -> &'a Links<T>;

    /// Getter for mutable links
    fn list_hook_mut<'a>(&'a mut self) -> &'a mut Links<T>;

    #[allow(visible_private_types)]
    fn next(&self) -> RawLink<T> {
        self.list_hook().next
    }

    #[allow(visible_private_types)]
    fn next_mut<'a>(&'a mut self) -> &'a mut RawLink<T> {
        &mut self.list_hook_mut().next
    }

    #[allow(visible_private_types)]
    fn prev(&self) -> RawLink<T> {
        self.list_hook().prev
    }

    #[allow(visible_private_types)]
    fn prev_mut<'a>(&'a mut self) -> &'a mut RawLink<T> {
        &mut self.list_hook_mut().prev
    }
}

/// An iterator over references to the items of a `DList`
pub struct Items<'a, T> {
    head: RawLink<T>,
    tail: RawLink<T>,
    lifetime: ContravariantLifetime<'a>
}

impl<'a, T> Clone for Items<'a, T> {
    #[inline]
    fn clone(&self) -> Items<'a, T> { *self }
}

// /// An iterator over mutable references to the items of a `DList`
pub struct MutItems<'a, T: Node<T>> {
    head: RawLink<T>,
    tail: RawLink<T>,
    #[allow(dead_code)]
    list: &'a mut DList<T>,
}

pub struct MoveItems<T: Node<T>> {
    list: DList<T>
}

impl<T: Node<T>> Links<T> {
    #[inline]
    pub fn new() -> Links<T> {
        Links{next: RawLink::none(), prev: RawLink::none()}
    }
}

impl<T: Node<T>> RawLink<T> {
    /// Like Option::None for RawLink
    fn none() -> RawLink<T> {
        RawLink{ptr: ptr::mut_null()}
    }

    /// Like Option::Some for RawLink
    fn some(n: *mut T) -> RawLink<T> {
        RawLink{ptr: n}
    }

    fn is_none(&self) -> bool {
        self.ptr.is_null()
    }

    fn is_some(&self) -> bool {
        !self.is_none()
    }

    fn map(&self, f: |&T| -> RawLink<T>) -> RawLink<T> {
        if self.is_none() {
            RawLink::none()
        } else {
            f(unsafe {&*self.ptr })
        }
    }

    fn as_ptr(&self) -> *const T {
        self.ptr as *const T
    }

    fn as_mut_ptr(&self) -> *mut T {
        self.ptr
    }

    fn next(&self) -> RawLink<T> {
        debug_assert!(self.is_some());
        unsafe {(*self.ptr).next()}
    }

    fn next_mut<'a>(&'a mut self) -> &'a mut RawLink<T> {
        debug_assert!(self.is_some());
        unsafe {(*self.ptr).next_mut()}
    }

    fn prev(&self) -> RawLink<T> {
        debug_assert!(self.is_some());
        unsafe {(*self.ptr).prev()}
    }

    fn prev_mut<'a>(&'a mut self) -> &'a mut RawLink<T> {
        debug_assert!(self.is_some());
        unsafe {(*self.ptr).prev_mut()}
    }
}

impl<T> PartialEq for RawLink<T> {
    fn eq(&self, other: &RawLink<T>) -> bool {
        self.ptr == other.ptr
    }
}

impl<T> Eq for RawLink<T> {}

impl<T: Node<T>> DList<T> {
    fn front_link(&self) -> RawLink<T> {
        self.head
    }

    fn back_link(&self) -> RawLink<T> {
        self.front_link().map(|t| t.prev())
    }

    fn insert_after(prev: RawLink<T>, node: RawLink<T>) {
        let next = prev.next();
        DList::insert(prev, next, node);
    }

    fn insert_before(next: RawLink<T>, node: RawLink<T>) {
        let prev = next.prev();
        DList::insert(prev, next, node);
    }

    fn insert(mut prev: RawLink<T>,
              mut next: RawLink<T>,
              mut node: RawLink<T>) {
        *next.prev_mut() = node;
        *node.next_mut() = next;
        *node.prev_mut() = prev;
        *prev.next_mut() = node;
    }

    fn remove_link(&mut self, mut node: RawLink<T>) {
        if self.front_link() == node {
            if self.back_link() == node {
                // Removing the only node, set to none
                self.head = RawLink::none();
            } else {
                // Removing the head with at least one node still left
                self.head = node.next();
            }
        }
        *node.prev().next_mut() = node.next();
        *node.next().prev_mut() = node.prev();
        *node.next_mut() = RawLink::none();
        *node.prev_mut() = RawLink::none();
    }
}

impl<T: Node<T>> Collection for DList<T> {
    #[inline]
    fn is_empty(&self) -> bool {
        self.head.is_none()
    }

    #[inline]
    fn len(&self) -> uint {
        let mut v = 0;
        for _ in self.iter() {
            v +=1
        }
        v
    }
}

impl<T: Node<T>> Mutable for DList<T> {
    #[inline]
    fn clear(&mut self) {
        loop {
            if self.pop_front().is_null() {
                break
            }
        }
    }
}

impl<T: Node<T>> MutableSeq<T> for DList<T> {
    #[inline]
    fn push(&mut self, node: *mut T) {
        assert!(node.is_not_null());
        if self.head.is_none() {
            *unsafe{ (*node).next_mut() } = RawLink::some(node);
            *unsafe{ (*node).prev_mut() } = RawLink::some(node);
            self.head = RawLink::some(node);
        } else {
            DList::insert_after(self.back_link(), RawLink::some(node))
        }
    }

    #[inline]
    fn pop(&mut self) -> *mut T {
        if self.head.is_none() {
            ptr::mut_null()
        } else {
            let back = self.back_link();
            self.remove_link(back);
            back.as_mut_ptr()
        }
    }
}

impl<T: Node<T>> Deque<T> for DList<T> {
    #[inline]
    fn front(&self) -> *const T {
        self.front_link().as_ptr()
    }

    #[inline]
    fn front_mut(&mut self) -> *mut T {
        self.front_link().as_mut_ptr()
    }

    #[inline]
    fn back(&self) -> *const T {
        self.back_link().as_ptr()
    }

    #[inline]
    fn back_mut(&mut self) -> *mut T {
        self.back_link().as_mut_ptr()
    }

    #[inline]
    fn push_front(&mut self, node: *mut T) {
        if self.head.is_none() {
            *unsafe{ (*node).next_mut() } = RawLink::some(node);
            *unsafe{ (*node).prev_mut() } = RawLink::some(node);
        } else {
            DList::insert_before(self.head, RawLink::some(node));
        }
        self.head = RawLink::some(node);
    }

    #[inline]
    fn pop_front(&mut self) -> *mut T {
        if self.head.is_none() {
            ptr::mut_null()
        } else {
            let front = self.front_link();
            self.remove_link(front);
            front.as_mut_ptr()
        }
    }
}

impl<T: Node<T>> Default for DList<T> {
    #[inline]
    fn default() -> DList<T> {
        DList::new()
    }
}

impl<T: Node<T>> DList<T> {
    /// Create an empty DList
    #[inline]
    pub fn new() -> DList<T> {
        DList{head: RawLink::none()}
    }

    /// Move the last element to the front of the list.
    ///
    /// If the list is empty, do nothing.
    #[inline]
    pub fn rotate_forward(&mut self) {
        if self.head.is_some() {
            self.head = self.back_link();
        }
    }

    /// Move the first element to the back of the list.
    ///
    /// If the list is empty, do nothing.
    #[inline]
    pub fn rotate_backward(&mut self) {
        if self.head.is_some() {
            self.head = self.head.next();
        }
    }

    /// Add all elements from `other` to the end of the list
    ///
    /// O(1)
    #[inline]
    pub fn append(&mut self, mut other: DList<T>) {
        if self.head.is_none() {
            *self = other
        } else {
            let mut o_front = other.front_link();
            let mut o_back = other.back_link();
            other.head = RawLink::none();
            if o_front.is_some() {
                *o_front.prev_mut() = self.back_link();
                *o_back.next_mut() = self.front_link();
                *self.back_link().next_mut() = o_front;
                *self.front_link().prev_mut() = o_back;
            }
        }
    }

    /// Add all elements from `other` to the beginning of hte list
    ///
    /// O(1)
    #[inline]
    pub fn prepend(&mut self, mut other: DList<T>) {
        mem::swap(self, &mut other);
        self.append(other);
    }

    /// Insert `node` before the first `x` in the list where `f(x, node)` is
    /// true, or at the end.
    ///
    /// O(N)
    #[inline]
    pub fn insert_when(&mut self, node: *mut T, f: |&T, &T| -> bool) {
        assert!(node.is_not_null());
        let mut it = self.mut_iter();
        loop {
            let next = it.peek_next();
            if next.is_null() || unsafe { f(&*next, &*node) } {
                break
            }
            it.next();
        }
        it.insert_next(node);
    }

    /// Remove an element from the list and return it
    ///
    /// # Safety Note
    ///
    /// The user must guarantee that node is in this list
    #[inline]
    pub unsafe fn remove(&mut self, node: *mut T) -> *mut T {
        assert!(node.is_not_null());
        self.remove_link(RawLink::some(node));
        node
    }

    /// Provide a forward iterator
    #[inline]
    pub fn iter<'a>(&'a self) -> Items<'a, T> {
        Items{
            head: self.front_link(),
            tail: self.back_link(),
            lifetime: ContravariantLifetime::<'a>
        }
    }

    /// Provide a forward iterator with mutable references
    #[inline]
    pub fn mut_iter<'a>(&'a mut self) -> MutItems<'a, T> {
        MutItems{
            head: self.front_link(),
            tail: self.back_link(),
            list: self,
        }
    }

    // Consume the list into an iterator yielding elements by value
    #[inline]
    pub fn move_iter(self) -> MoveItems<T> {
        MoveItems{list: self}
    }
}

impl<T: Node<T> + Ord> DList<T> {
    /// Insert `node` sorted in ascending order
    ///
    /// O(N)
    #[inline]
    pub fn insert_ordered(&mut self, node: *mut T) {
        self.insert_when(node, |a, b| a >= b)
    }
}

impl<'a, T: Node<T>> Iterator<*const T> for Items<'a, T> {
    #[inline]
    fn next(&mut self) -> Option<*const T> {
        if self.head.is_none() {
            return None;
        }
        let ret = self.head.as_ptr();
        if self.head == self.tail {
            self.head = RawLink::none();
            self.tail = RawLink::none();
        } else {
            self.head = unsafe { (*ret).next() };
        }
        Some(ret)
    }
}

impl<'a, T: Node<T>> DoubleEndedIterator<*const T> for Items<'a, T> {
    #[inline]
    fn next_back(&mut self) -> Option<*const T> {
        if self.tail.is_none() {
            return None;
        }
        let ret = self.tail.as_ptr();
        if self.head == self.tail {
            self.head = RawLink::none();
            self.tail = RawLink::none();
        } else {
            self.tail = unsafe { (*ret).prev() };
        }
        Some(ret)
    }
}

impl<'a, T: Node<T>> Iterator<*mut T> for MutItems<'a, T> {
    #[inline]
    fn next(&mut self) -> Option<*mut T> {
        if self.head.is_none() {
            return None;
        }
        let ret = self.head.as_mut_ptr();
        if self.head == self.tail {
            self.head = RawLink::none();
            self.tail = RawLink::none();
        } else {
            self.head = unsafe { (*ret).next() };
        }
        Some(ret)
    }
}

impl<'a, T: Node<T>> DoubleEndedIterator<*mut T> for MutItems<'a, T> {
    #[inline]
    fn next_back(&mut self) -> Option<*mut T> {
        if self.tail.is_none() {
            return None;
        }
        let ret = self.tail.as_mut_ptr();
        if self.head == self.tail {
            self.head = RawLink::none();
            self.tail = RawLink::none();
        } else {
            self.tail = unsafe { (*ret).prev() };
        }
        Some(ret)
    }
}

/// Allow mutating the DList while iterating the list.
pub trait ListInsertion<T> {
    /// Insert `node` just after to the element most recently return by `next()`
    ///
    /// The inserted element does not appear in the iteration.
    fn insert_next(&mut self, node: *mut T);

    /// Provide a pointer to the next element, without changing the iterator.
    ///
    /// The pointer is null if there is no next element
    fn peek_next(&mut self) -> *mut T;
}

impl<'a, T: Node<T>> ListInsertion<T> for MutItems<'a, T> {
    #[inline]
    fn insert_next(&mut self, node: *mut T) {
        assert!(node.is_not_null());
        DList::insert_before(self.head, RawLink::some(node));
    }

    #[inline]
    fn peek_next(&mut self) -> *mut T {
        self.head.as_mut_ptr()
    }
}

impl<T: Node<T>> Iterator<*mut T> for MoveItems<T> {
    #[inline]
    fn next(&mut self) -> Option<*mut T> {
        let front = self.list.pop_front();
        if front.is_null() {
            None
        } else {
            Some(front)
        }
    }
}

impl<T: Node<T>> DoubleEndedIterator<*mut T> for MoveItems<T> {
    #[inline]
    fn next_back(&mut self) -> Option<*mut T> {
        let back = self.list.pop();
        if back.is_null() {
            None
        } else {
            Some(back)
        }
    }
}

impl<T: Node<T> + PartialEq> PartialEq for DList<T> {
    #[inline]
    fn eq(&self, other: &DList<T>) -> bool {
        self.len() == other.len() &&
            iter::order::eq(self.iter().map(|node| unsafe{ptr::read(node)}),
                            other.iter().map(|node| unsafe{ptr::read(node)}))
    }

    #[inline]
    fn ne(&self, other: &DList<T>) -> bool {
        self.len() != other.len() ||
            iter::order::ne(self.iter().map(|node| unsafe{ptr::read(node)}),
                            other.iter().map(|node| unsafe{ptr::read(node)}))
    }
}

impl<T: Node<T> + Eq> Eq for DList<T> {}

impl<T: Node<T> + PartialOrd> PartialOrd for DList<T> {
    #[inline]
    fn partial_cmp(&self, other: &DList<T>) -> Option<Ordering> {
        iter::order::partial_cmp(self.iter().map(|node| unsafe{ptr::read(node)}),
                                 other.iter().map(|node| unsafe{ptr::read(node)}))
    }
}

impl<T: Node<T> + Ord> Ord for DList<T> {
    #[inline]
    fn cmp(&self, other: &DList<T>) -> Ordering {
        iter::order::cmp(self.iter().map(|node| unsafe{ptr::read(node)}),
                         other.iter().map(|node| unsafe{ptr::read(node)}))
    }
}

impl<T: Node<T> + fmt::Show> fmt::Show for DList<T> {
    #[inline]
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        try!(write!(f, "["));

        for (i, e) in self.iter().enumerate() {
            if i != 0 { try!(write!(f, ", ")); }
            try!(write!(f, "{}", unsafe{ ptr::read(e) }));
        }

        write!(f, "]")
    }
}

#[cfg(test)]
mod test {
    use std::mem;
    use std::prelude::*;
    use std::ptr;

    use Deque as IntrusiveDeque;
    use MutableSeq as IntrusiveMutableSeq;
    use Mutable as IntrusiveMutable;
    use super::{DList, Links, Node, RawLink};

    struct MyNode {
        list_hook: Links<MyNode>,
        val: int
    }

    impl Node<MyNode> for MyNode {
        fn list_hook<'a>(&'a self) -> &'a Links<MyNode> {
            &self.list_hook
        }
        fn list_hook_mut<'a>(&'a mut self) -> &'a mut Links<MyNode> {
            &mut self.list_hook
        }
    }

    impl Clone for MyNode {
        fn clone(&self) -> MyNode {
            MyNode {
                list_hook: Links::new(),
                val: self.val
            }
        }
    }

    impl PartialEq for MyNode {
        fn eq(&self, other: &MyNode) -> bool {
            self.val == other.val
        }
    }

    impl Eq for MyNode {}

    pub fn check_links<T: Node<T>>(list: &DList<T>) {
        if list.head.is_none() {
            return;
        }

        let mut len = 0u;
        let tail = list.head.prev();
        let mut last_ptr: RawLink<T> = tail;
        let mut node_ptr: RawLink<T> = list.head;
        loop {
            len += 1;
            assert!(last_ptr == node_ptr.prev());
            if node_ptr == tail {
                break;
            }
            last_ptr = node_ptr;
            node_ptr = node_ptr.next();
        }

        assert_eq!(len, list.len());
    }

    #[test]
    fn test_basic() {
        unsafe {
            let mut m: DList<MyNode> = DList::new();
            assert_eq!(m.pop_front(), ptr::mut_null());
            assert_eq!(m.pop(), ptr::mut_null());
            assert_eq!(m.pop(), ptr::mut_null());
            let v1_box = box MyNode {list_hook: Links::new(), val: 1};
            let v1 : *mut MyNode = mem::transmute(v1_box);
            m.push_front(v1);
            assert_eq!(m.pop_front(), v1);
            let v2_box = box MyNode {list_hook: Links::new(), val: 2};
            let v2 : *mut MyNode = mem::transmute(v2_box);
            m.push(v2);
            let v3_box = box MyNode {list_hook: Links::new(), val: 3};
            let v3 : *mut MyNode = mem::transmute(v3_box);
            m.push(v3);
            assert_eq!(m.len(), 2);
            assert_eq!(m.pop_front(), v2);
            assert_eq!(m.pop_front(), v3);
            assert_eq!(m.len(), 0);
            assert_eq!(m.pop_front(), ptr::mut_null());
            let v1_box = box MyNode {list_hook: Links::new(), val: 1};
            let v1 : *mut MyNode = mem::transmute(v1_box);
            m.push(v1);
            let v3_box = box MyNode {list_hook: Links::new(), val: 3};
            let v3 : *mut MyNode = mem::transmute(v3_box);
            m.push(v3);
            let v5_box = box MyNode {list_hook: Links::new(), val: 5};
            let v5 : *mut MyNode = mem::transmute(v5_box);
            m.push(v5);
            let v7_box = box MyNode {list_hook: Links::new(), val: 7};
            let v7 : *mut MyNode = mem::transmute(v7_box);
            m.push(v7);
            assert_eq!(m.pop_front(), v1);

            let mut n = DList::new();
            let v2_box = box MyNode {list_hook: Links::new(), val: 2};
            let v2 : *mut MyNode = mem::transmute(v2_box);
            n.push_front(v2);
            let v3_box = box MyNode {list_hook: Links::new(), val: 3};
            let v3 : *mut MyNode = mem::transmute(v3_box);
            n.push_front(v3);
            {
                assert_eq!(n.front(), v3 as *const MyNode);
                let x = n.front_mut();
                assert_eq!((*x).val, 3);
                (*x).val = 0;
            }
            {
                assert_eq!(n.back(), v2 as *const MyNode);
                let x = n.back_mut();
                assert_eq!((*x).val, 2);
                (*x).val = 0;
            }
        }
    }

    #[cfg(test)]
    unsafe fn list_from<T: Node<T>>(v: &[T]) -> DList<T> {
        let mut ret = DList::new();
        for t in v.iter() {
            ret.push(mem::transmute(t));
        }
        ret
    }

    #[test]
    fn test_append() {
        {
            let mut m = DList::new();
            let mut n = DList::new();
            let mut v2_box = box MyNode {list_hook: Links::new(), val: 2};
            let v2 : *mut MyNode = &mut *v2_box;
            n.push(v2);
            m.append(n);
            assert_eq!(m.len(), 1);
            assert_eq!(m.pop(), v2);
            check_links(&m);
        }
        {
            let mut m = DList::new();
            let n = DList::new();
            let mut v2_box = box MyNode {list_hook: Links::new(), val: 2};
            let v2 : *mut MyNode = &mut *v2_box;
            m.push(v2);
            m.append(n);
            assert_eq!(m.len(), 1);
            assert_eq!(m.pop(), v2);
            check_links(&m);
        }

        let v = vec![MyNode{ list_hook: Links::new(), val: 1},
                     MyNode{ list_hook: Links::new(), val: 2},
                     MyNode{ list_hook: Links::new(), val: 3},
                     MyNode{ list_hook: Links::new(), val: 4},
                     MyNode{ list_hook: Links::new(), val: 5}];
        let u = vec![MyNode{ list_hook: Links::new(), val: 9},
                     MyNode{ list_hook: Links::new(), val: 8},
                     MyNode{ list_hook: Links::new(), val: 1},
                     MyNode{ list_hook: Links::new(), val: 2},
                     MyNode{ list_hook: Links::new(), val: 3},
                     MyNode{ list_hook: Links::new(), val: 4},
                     MyNode{ list_hook: Links::new(), val: 5}];
        let mut m = unsafe {list_from(v.as_slice())};
        m.append(unsafe {list_from(u.as_slice())} );
        check_links(&m);
        m.clear();
    }

    #[test]
    fn test_prepend() {
        {
            let mut m = DList::new();
            let mut n = DList::new();
            let mut v2_box = box MyNode {list_hook: Links::new(), val: 2};
            let v2 : *mut MyNode = &mut *v2_box;
            n.push(v2);
            m.prepend(n);
            assert_eq!(m.len(), 1);
            assert_eq!(m.pop(), v2);
            check_links(&m);
        }

        let v = vec![MyNode{ list_hook: Links::new(), val: 1},
                     MyNode{ list_hook: Links::new(), val: 2},
                     MyNode{ list_hook: Links::new(), val: 3},
                     MyNode{ list_hook: Links::new(), val: 4},
                     MyNode{ list_hook: Links::new(), val: 5}];
        let u = vec![MyNode{ list_hook: Links::new(), val: 9},
                     MyNode{ list_hook: Links::new(), val: 8},
                     MyNode{ list_hook: Links::new(), val: 1},
                     MyNode{ list_hook: Links::new(), val: 2},
                     MyNode{ list_hook: Links::new(), val: 3},
                     MyNode{ list_hook: Links::new(), val: 4},
                     MyNode{ list_hook: Links::new(), val: 5}];
        let mut m = unsafe {list_from(v.as_slice())};
        m.prepend(unsafe {list_from(u.as_slice())} );
        check_links(&m);
        m.clear();
    }

    #[test]
    fn test_rotate() {
        let mut n: DList<MyNode> = DList::new();
        n.rotate_backward(); check_links(&n);
        assert_eq!(n.len(), 0);
        n.rotate_forward(); check_links(&n);
        assert_eq!(n.len(), 0);

        let v = vec![MyNode{ list_hook: Links::new(), val: 1},
                     MyNode{ list_hook: Links::new(), val: 2},
                     MyNode{ list_hook: Links::new(), val: 3},
                     MyNode{ list_hook: Links::new(), val: 4},
                     MyNode{ list_hook: Links::new(), val: 5}];
        let mut m = unsafe {list_from(v.as_slice())};
        m.rotate_backward(); check_links(&m);
        m.rotate_forward(); check_links(&m);
        assert_eq!(v.iter().map(|x| unsafe {mem::transmute(x)}).
                   collect::<Vec<*const MyNode>>(), m.iter().collect());
        m.rotate_forward(); check_links(&m);
        m.rotate_forward(); check_links(&m);
        m.pop_front(); check_links(&m);
        m.rotate_forward(); check_links(&m);
        m.rotate_backward(); check_links(&m);
        m.clear();
    }

    #[test]
    fn test_iterator() {
        let v = vec![MyNode{ list_hook: Links::new(), val: 0},
                     MyNode{ list_hook: Links::new(), val: 1},
                     MyNode{ list_hook: Links::new(), val: 2},
                     MyNode{ list_hook: Links::new(), val: 3},
                     MyNode{ list_hook: Links::new(), val: 4},
                     MyNode{ list_hook: Links::new(), val: 5},
                     MyNode{ list_hook: Links::new(), val: 6}];
        let mut m = unsafe {list_from(v.as_slice())};
        for (i, elt) in m.iter().enumerate() {
            assert_eq!(i as int, unsafe {(*elt).val});
        }
        m.clear();
        let mut n = DList::new();
        assert_eq!(n.iter().next(), None)
        let mut v4_box = box MyNode {list_hook: Links::new(), val: 4};
        let v4 : *mut MyNode = &mut *v4_box;
        n.push_front(v4);
        {
            let mut it = n.iter();
            assert_eq!(it.next().unwrap(), v4 as *const MyNode);
            assert_eq!(it.next(), None)
        }
        n.clear();
    }

    #[test]
    fn test_iterator_clone() {
        let mut n = DList::new();
        let mut v2_box = box MyNode {list_hook: Links::new(), val: 2};
        let v2 : *mut MyNode = &mut *v2_box;
        n.push(v2);
        let mut v3_box = box MyNode {list_hook: Links::new(), val: 3};
        let v3 : *mut MyNode = &mut *v3_box;
        n.push(v3);
        let mut v4_box = box MyNode {list_hook: Links::new(), val: 4};
        let v4 : *mut MyNode = &mut *v4_box;
        n.push(v4);
        {
            let mut it = n.iter();
            it.next();
            let mut jt = it.clone();
            assert_eq!(it.next(), jt.next());
            assert_eq!(it.next_back(), jt.next_back());
            assert_eq!(it.next(), jt.next());
        }
        n.clear();
    }

    #[test]
    fn test_iterator_double_end() {
        let mut n = DList::new();
        assert_eq!(n.iter().next(), None);
        let mut v4_box = box MyNode {list_hook: Links::new(), val: 4};
        let v4 : *mut MyNode = &mut *v4_box;
        n.push_front(v4);
        let mut v5_box = box MyNode {list_hook: Links::new(), val: 5};
        let v5 : *mut MyNode = &mut *v5_box;
        n.push_front(v5);
        let mut v6_box = box MyNode {list_hook: Links::new(), val: 6};
        let v6 : *mut MyNode = &mut *v6_box;
        n.push_front(v6);
        {
            let mut it = n.iter();
            assert_eq!(it.next().unwrap(), v6 as *const MyNode);
            assert_eq!(it.next_back().unwrap(), v4 as *const MyNode);
            assert_eq!(it.next_back().unwrap(), v5 as *const MyNode);
            assert_eq!(it.next_back(), None);
            assert_eq!(it.next(), None);
        }
        n.clear();
    }

    #[test]
    fn test_rev_iter() {
        let v = vec![MyNode{ list_hook: Links::new(), val: 0},
                     MyNode{ list_hook: Links::new(), val: 1},
                     MyNode{ list_hook: Links::new(), val: 2},
                     MyNode{ list_hook: Links::new(), val: 3},
                     MyNode{ list_hook: Links::new(), val: 4},
                     MyNode{ list_hook: Links::new(), val: 5},
                     MyNode{ list_hook: Links::new(), val: 6}];
        let mut m = unsafe {list_from(v.as_slice())};
        for (i, elt) in m.iter().rev().enumerate() {
            assert_eq!((6 - i) as int, unsafe {(*elt).val});
        }
        m.clear();
        let mut n = DList::new();
        assert_eq!(n.iter().rev().next(), None);
        let mut v4_box = box MyNode {list_hook: Links::new(), val: 4};
        let v4 : *mut MyNode = &mut *v4_box;
        n.push_front(v4);
        {
            let mut it = n.iter().rev();
            assert_eq!(it.next().unwrap(), v4 as *const MyNode);
            assert_eq!(it.next(), None);
        }
        n.clear();
    }

    #[test]
    fn test_mut_iter() {
        let v = vec![MyNode{ list_hook: Links::new(), val: 0},
                     MyNode{ list_hook: Links::new(), val: 1},
                     MyNode{ list_hook: Links::new(), val: 2},
                     MyNode{ list_hook: Links::new(), val: 3},
                     MyNode{ list_hook: Links::new(), val: 4},
                     MyNode{ list_hook: Links::new(), val: 5},
                     MyNode{ list_hook: Links::new(), val: 6}];
        let mut m = unsafe {list_from(v.as_slice())};
        let mut len = m.len();
        for (i, elt) in m.mut_iter().enumerate() {
            assert_eq!(i as int, unsafe {(*elt).val});
            len -= 1;
        }
        assert_eq!(len, 0);
        m.clear();
        let mut n = DList::new();
        assert!(n.mut_iter().next().is_none());
        let mut v4_box = box MyNode {list_hook: Links::new(), val: 4};
        let v4 : *mut MyNode = &mut *v4_box;
        n.push_front(v4);
        let mut v5_box = box MyNode {list_hook: Links::new(), val: 5};
        let v5 : *mut MyNode = &mut *v5_box;
        n.push(v5);
        {
            let mut it = n.mut_iter();
            assert!(it.next().is_some());
            assert!(it.next().is_some());
            assert!(it.next().is_none());
        }
        n.clear();
    }

    #[test]
    fn test_iterator_mut_double_end() {
        let mut n = DList::new();
        assert!(n.mut_iter().next_back().is_none());
        let mut v4_box = box MyNode {list_hook: Links::new(), val: 4};
        let v4 : *mut MyNode = &mut *v4_box;
        n.push_front(v4);
        let mut v5_box = box MyNode {list_hook: Links::new(), val: 5};
        let v5 : *mut MyNode = &mut *v5_box;
        n.push_front(v5);
        let mut v6_box = box MyNode {list_hook: Links::new(), val: 6};
        let v6 : *mut MyNode = &mut *v6_box;
        n.push_front(v6);
        {
            let mut it = n.mut_iter();
            assert_eq!(it.next().unwrap(), v6);
            assert_eq!(it.next_back().unwrap(), v4);
            assert_eq!(it.next_back().unwrap(), v5);
            assert!(it.next_back().is_none());
            assert!(it.next().is_none());
        }
        n.clear();
    }

    #[test]
    #[should_fail]
    fn test_lifetime() {
        let mut n = DList::new();
        {
            let mut v1_box = box MyNode {list_hook: Links::new(), val: 1};
            let v1 : *mut MyNode = &mut *v1_box;
            n.push(v1);
        }
    }

    #[test]
    fn test_lifetime2() {
        let mut n = DList::new();
        {
            let mut v1_box = box MyNode {list_hook: Links::new(), val: 1};
            let v1 : *mut MyNode = &mut *v1_box;
            n.push(v1);
            n.pop();
        }
    }
}
