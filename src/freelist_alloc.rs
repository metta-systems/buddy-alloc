//! Freelist allocator
//! Optimized for fixed small memory block.

use core::{
    alloc::{AllocError, Allocator, Layout},
    cell::RefCell,
    ptr::NonNull,
};

/// Fixed size 64 Bytes, can't allocate more in one allocation.
pub const BLOCK_SIZE: usize = 64;

struct Node {
    next: *mut Node,
    prev: *mut Node,
}

impl Node {
    fn init(list: *mut Node) {
        unsafe {
            (*list).next = list;
            (*list).prev = list;
        }
    }

    fn remove(list: *mut Node) {
        unsafe {
            (*(*list).prev).next = (*list).next;
            (*(*list).next).prev = (*list).prev;
        }
    }

    fn pop(list: *mut Node) -> *mut Node {
        let n_list: *mut Node = unsafe { (*list).next };
        Self::remove(n_list);
        n_list
    }

    fn push(list: *mut Node, p: *mut u8) {
        let p = p.cast::<Node>();
        unsafe {
            let n_list: Node = Node {
                prev: list,
                next: (*list).next,
            };
            p.write_unaligned(n_list);
            (*(*list).next).prev = p;
            (*list).next = p;
        }
    }

    fn is_empty(list: *const Node) -> bool {
        unsafe { (*list).next as *const Node == list }
    }
}

#[derive(Clone, Copy)]
pub struct FreelistAllocParam {
    base_addr: *const u8,
    len: usize,
}

impl FreelistAllocParam {
    pub const fn new(base_addr: *const u8, len: usize) -> Self {
        FreelistAllocParam { base_addr, len }
    }
}

pub struct FreelistAlloc {
    /// memory start addr
    base_addr: usize,
    /// memory end addr
    end_addr: usize,
    free: RefCell<*mut Node>,
}

impl FreelistAlloc {
    /// # Safety
    ///
    /// The `base_addr..(base_addr + len)` must be allocated before use,
    /// and must guarantee no others write to the memory range, otherwise behavior is undefined.
    pub unsafe fn new(param: FreelistAllocParam) -> Self {
        let FreelistAllocParam { base_addr, len } = param;
        let base_addr = base_addr as usize;
        let end_addr = base_addr + len;
        debug_assert_eq!(len % BLOCK_SIZE, 0);

        let nblocks = len / BLOCK_SIZE;

        // initialize free list
        let free = base_addr as *mut Node;
        Node::init(free);

        let mut addr = base_addr;
        for _ in 0..(nblocks - 1) {
            addr += BLOCK_SIZE;
            Node::push(free, addr as *mut u8);
        }

        FreelistAlloc {
            base_addr,
            end_addr,
            free: RefCell::new(free),
        }
    }

    pub fn contains_ptr(&self, p: *mut u8) -> bool {
        let addr = p as usize;
        addr >= self.base_addr && addr < self.end_addr
    }
}

unsafe impl Allocator for FreelistAlloc {
    fn allocate(&self, layout: Layout) -> Result<NonNull<[u8]>, AllocError> {
        let nbytes = layout.size();
        // TODO: alignment!
        if nbytes > BLOCK_SIZE || self.free.borrow().is_null() {
            return Err(AllocError);
        }

        let is_last = Node::is_empty(self.free.borrow().cast_const());
        let p = Node::pop(*self.free.borrow_mut()) as *mut u8;
        if is_last {
            self.free.replace(core::ptr::null_mut());
        }
        Ok(NonNull::slice_from_raw_parts(
            unsafe { NonNull::new_unchecked(p) },
            layout.size(),
        ))
    }

    unsafe fn deallocate(&self, ptr: NonNull<u8>, _layout: Layout) {
        let p = ptr.as_ptr();
        debug_assert!(self.contains_ptr(p));
        let f = self.free.borrow();
        if f.is_null() {
            let n = p.cast();
            Node::init(n);
            drop(f);
            self.free.replace(n);
        } else {
            drop(f);
            Node::push(*self.free.borrow_mut(), p);
        }
    }
}
