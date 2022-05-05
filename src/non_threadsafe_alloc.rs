//! NonThreadSafeAlloc
//! An allocator that does not support thread-safe

use {
    crate::{
        buddy_alloc::{BuddyAlloc, BuddyAllocParam},
        freelist_alloc::{FreelistAlloc, FreelistAllocParam, BLOCK_SIZE},
    },
    core::{
        alloc::{AllocError, Allocator, GlobalAlloc, Layout},
        cell::RefCell,
        ptr::NonNull,
    },
};

/// Use buddy allocator if request bytes is large than this,
/// otherwise use freelist allocator
const MAX_FREELIST_ALLOC_SIZE: usize = BLOCK_SIZE;

/// NonThreadsafeAlloc
/// perfect for single threaded devices
pub struct NonThreadsafeAlloc {
    freelist_alloc_param: FreelistAllocParam,
    inner_freelist_alloc: RefCell<Option<FreelistAlloc>>,
    buddy_alloc_param: BuddyAllocParam,
    inner_buddy_alloc: RefCell<Option<BuddyAlloc>>,
}

impl NonThreadsafeAlloc {
    /// see BuddyAlloc::new
    pub const fn new(
        freelist_alloc_param: FreelistAllocParam,
        buddy_alloc_param: BuddyAllocParam,
    ) -> Self {
        NonThreadsafeAlloc {
            inner_freelist_alloc: RefCell::new(None),
            inner_buddy_alloc: RefCell::new(None),
            freelist_alloc_param,
            buddy_alloc_param,
        }
    }

    unsafe fn fetch_freelist_alloc<R, F: FnOnce(&mut FreelistAlloc) -> R>(&self, f: F) -> R {
        let mut inner = self.inner_freelist_alloc.borrow_mut();
        if inner.is_none() {
            inner.replace(FreelistAlloc::new(self.freelist_alloc_param));
        }
        f(inner.as_mut().expect("nerver"))
    }

    unsafe fn fetch_buddy_alloc<R, F: FnOnce(&mut BuddyAlloc) -> R>(&self, f: F) -> R {
        let mut inner = self.inner_buddy_alloc.borrow_mut();
        if inner.is_none() {
            inner.replace(BuddyAlloc::new(self.buddy_alloc_param));
        }
        f(inner.as_mut().expect("nerver"))
    }
}

// ==== Allocator api ====
unsafe impl Allocator for NonThreadsafeAlloc {
    /// Allocate a memory block from the pool.
    fn allocate(&self, layout: Layout) -> Result<NonNull<[u8]>, AllocError> {
        // use BuddyAlloc if size is larger than MAX_FREELIST_ALLOC_SIZE
        if layout.size() > MAX_FREELIST_ALLOC_SIZE {
            unsafe { self.fetch_buddy_alloc(|alloc| alloc.allocate(layout)) }
        } else {
            // try freelist alloc, fallback to BuddyAlloc if failed
            unsafe {
                self.fetch_freelist_alloc(|alloc| alloc.allocate(layout))
                    .or_else(|_| self.fetch_buddy_alloc(|alloc| alloc.allocate(layout)))
            }
        }
    }

    unsafe fn deallocate(&self, ptr: NonNull<u8>, layout: Layout) {
        let freed = self.fetch_freelist_alloc(|alloc| {
            if alloc.contains_ptr(ptr.as_ptr()) {
                alloc.deallocate(ptr, layout);
                true
            } else {
                false
            }
        });
        if !freed {
            self.fetch_buddy_alloc(|alloc| alloc.deallocate(ptr, layout));
        }
    }
}

// ==== GlobalAlloc api ====
unsafe impl Sync for NonThreadsafeAlloc {}

unsafe impl GlobalAlloc for NonThreadsafeAlloc {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        self.allocate(layout)
            .map_or(core::ptr::null_mut(), |p| p.as_mut_ptr())
    }

    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        if !ptr.is_null() {
            self.deallocate(NonNull::new_unchecked(ptr), layout)
        }
    }
}
