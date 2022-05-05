use {
    crate::freelist_alloc::{FreelistAlloc, FreelistAllocParam, BLOCK_SIZE},
    core::alloc::{Allocator, Layout},
};

fn with_allocator<F: FnOnce(FreelistAlloc)>(f: F, buf: &[u8]) {
    let allocator = unsafe {
        let addr = buf.as_ptr();
        let len = buf.len();
        let param = FreelistAllocParam::new(addr, len);
        FreelistAlloc::new(param)
    };
    f(allocator);
}

#[test]
fn test_basic_malloc() {
    let buf = [0u8; 4096];
    // alloc a min block
    with_allocator(
        |allocator| {
            let p = allocator.allocate(Layout::from_size_align(64, 1).unwrap());
            assert!(p.is_ok());
            let p = p.unwrap();
            let p_addr = p.as_mut_ptr() as usize;
            // memory writeable
            unsafe { p.as_mut_ptr().write(42) };
            assert_eq!(p_addr, p.as_mut_ptr() as usize);
            assert_eq!(unsafe { *p.as_mut_ptr() }, 42);
        },
        &buf,
    );
}

#[test]
fn test_multiple_malloc() {
    let buf = [0u8; 4096];
    with_allocator(
        |allocator| {
            let mut available_bytes = buf.len();
            // alloc serveral sized blocks
            while available_bytes >= BLOCK_SIZE {
                let bytes = BLOCK_SIZE;
                assert!(allocator
                    .allocate(Layout::from_size_align(bytes, 1).unwrap())
                    .is_ok());
                available_bytes -= bytes;
            }
        },
        &buf,
    );
}

#[test]
fn test_small_size_malloc() {
    let buf = [0u8; 4096];
    with_allocator(
        |allocator| {
            let mut available_bytes = buf.len();
            while available_bytes >= BLOCK_SIZE {
                assert!(allocator
                    .allocate(Layout::from_size_align(BLOCK_SIZE, 1).unwrap())
                    .is_ok());
                available_bytes -= BLOCK_SIZE;
            }
            // memory should be drained, we can't allocate even 1 byte
            assert!(allocator
                .allocate(Layout::from_size_align(1, 1).unwrap())
                .is_err());
        },
        &buf,
    );
}

#[test]
fn test_fail_malloc() {
    let buf = [0u8; 4096];
    // not enough memory since we only have HEAP_SIZE bytes,
    // and the allocator itself occupied few bytes
    with_allocator(
        |allocator| {
            let p = allocator.allocate(Layout::from_size_align(BLOCK_SIZE + 1, 1).unwrap());
            assert!(p.is_err());
        },
        &buf,
    );
}

#[test]
fn test_malloc_and_free() {
    fn _test_malloc_and_free(times: usize) {
        let buf = [0u8; 4096];
        with_allocator(
            |allocator| {
                for _i in 0..times {
                    let mut available_bytes = buf.len();
                    let mut ptrs = Vec::new();
                    // alloc serveral sized blocks
                    while available_bytes >= BLOCK_SIZE {
                        let bytes = BLOCK_SIZE;
                        let p = allocator.allocate(Layout::from_size_align(bytes, 1).unwrap());
                        assert!(p.is_ok());
                        ptrs.push(p.unwrap());
                        available_bytes -= bytes;
                    }
                    // space is drained
                    assert!(allocator
                        .allocate(Layout::from_size_align(1, 1).unwrap())
                        .is_err());
                    // free allocated blocks
                    for ptr in ptrs {
                        assert!(allocator.contains_ptr(ptr.as_mut_ptr()));
                        unsafe {
                            allocator.deallocate(ptr.cast(), Layout::from_size_align(1, 1).unwrap())
                        };
                    }
                }
            },
            &buf,
        );
    }
    _test_malloc_and_free(10);
}

#[test]
fn test_free_bug() {
    let buf = [0u8; 4096];
    with_allocator(
        |allocator| {
            let layout = Layout::from_size_align(32, 1).unwrap();
            let p1 = allocator.allocate(layout).unwrap();
            unsafe { allocator.deallocate(p1.cast(), layout) };
            let layout = Layout::from_size_align(64, 1).unwrap();
            let p2 = allocator.allocate(layout).unwrap();
            let layout2 = Layout::from_size_align(61, 1).unwrap();
            let p3 = allocator.allocate(layout2).unwrap();
            unsafe { allocator.deallocate(p2.cast(), layout) };
            unsafe { allocator.deallocate(p3.cast(), layout2) };
        },
        &buf,
    );
}
