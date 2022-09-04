use {
    crate::buddy_alloc::{block_size, BuddyAlloc, BuddyAllocParam, MIN_LEAF_SIZE_ALIGN},
    core::alloc::{Allocator, Layout},
};

const HEAP_SIZE: usize = 1024 * 1024;
const LEAF_SIZE: usize = MIN_LEAF_SIZE_ALIGN;

fn with_allocator<F: FnOnce(BuddyAlloc)>(heap_size: usize, leaf_size: usize, f: F) {
    let buf: Vec<u8> = Vec::with_capacity(heap_size);
    let param = BuddyAllocParam::new(buf.as_ptr(), heap_size, leaf_size);
    unsafe {
        let allocator = BuddyAlloc::new(param);
        f(allocator);
    }
}

// find a max k that less than n bytes
pub fn first_down_k(n: usize) -> Option<usize> {
    let mut k: usize = 0;
    let mut size = LEAF_SIZE;
    while size < n {
        k += 1;
        size *= 2;
    }
    if size != n {
        k.checked_sub(1)
    } else {
        Some(k)
    }
}

#[test]
fn test_available_bytes() {
    with_allocator(HEAP_SIZE, LEAF_SIZE, |allocator| {
        let available_bytes = allocator.available_bytes();
        assert!(available_bytes > (HEAP_SIZE as f64 * 0.8) as usize);
    });
}

#[test]
fn test_basic_malloc() {
    // alloc a min block
    with_allocator(HEAP_SIZE, LEAF_SIZE, |allocator| {
        let p = allocator.allocate(Layout::from_size_align(512, 8).unwrap());
        assert!(p.is_ok());
        // memory writeable
        let p = p.unwrap().as_mut_ptr();
        let p_addr = p as usize;
        unsafe { p.write(42) };
        assert_eq!(p_addr, p as usize);
        assert_eq!(unsafe { *p }, 42);
    });
}

#[test]
fn test_multiple_malloc() {
    with_allocator(HEAP_SIZE, LEAF_SIZE, |allocator| {
        let mut available_bytes = allocator.available_bytes();
        let mut count = 0;
        // alloc serveral sized blocks
        while available_bytes >= LEAF_SIZE {
            let k = first_down_k(available_bytes - 1).unwrap_or_default();
            let bytes = block_size(k, LEAF_SIZE);
            assert!(allocator
                .allocate(Layout::from_size_align(bytes, 1).unwrap())
                .is_ok());
            available_bytes -= bytes;
            count += 1;
        }
        assert_eq!(count, 11);
    });
}

#[test]
fn test_small_size_malloc() {
    with_allocator(HEAP_SIZE, LEAF_SIZE, |allocator| {
        let mut available_bytes = allocator.available_bytes();
        while available_bytes >= LEAF_SIZE {
            assert!(allocator
                .allocate(Layout::from_size_align(LEAF_SIZE, 1).unwrap())
                .is_ok());
            available_bytes -= LEAF_SIZE;
        }
        // memory should be drained, we can't allocate even 1 byte
        assert!(allocator
            .allocate(Layout::from_size_align(1, 1).unwrap())
            .is_err());
    });
}

#[test]
fn test_fail_malloc() {
    // not enough memory since we only have HEAP_SIZE bytes,
    // and the allocator itself occupied few bytes
    with_allocator(HEAP_SIZE, LEAF_SIZE, |allocator| {
        let p = allocator.allocate(Layout::from_size_align(HEAP_SIZE, 1).unwrap());
        assert!(p.is_err());
    });
}

#[test]
fn test_malloc_and_free() {
    fn _test_malloc_and_free(times: usize, heap_size: usize) {
        with_allocator(heap_size, LEAF_SIZE, |allocator| {
            for _i in 0..times {
                let mut available_bytes = allocator.available_bytes();
                let mut ptrs = Vec::new();
                let mut layouts = Vec::new();
                // alloc several sized blocks
                while available_bytes >= LEAF_SIZE {
                    let k = first_down_k(available_bytes - 1).unwrap_or_default();
                    let bytes = block_size(k, LEAF_SIZE);
                    let layout = Layout::from_size_align(bytes, 1).unwrap();
                    let p = allocator.allocate(layout);
                    assert!(p.is_ok());
                    ptrs.push(p.unwrap());
                    layouts.push(layout);
                    available_bytes -= bytes;
                }
                // space is drained
                assert!(allocator
                    .allocate(Layout::from_size_align(1, 1).unwrap())
                    .is_err());
                // free allocated blocks
                for ptr in ptrs {
                    let layout = layouts.pop().unwrap();
                    unsafe {
                        allocator.deallocate(ptr.cast(), layout);
                    }
                }
            }
        });
    }
    // test with heaps: 1M, 2M, 4M, 8M
    for i in &[1, 2, 4, 8] {
        _test_malloc_and_free(10, i * HEAP_SIZE);
    }
}

#[test]
fn test_free_bug() {
    with_allocator(HEAP_SIZE, LEAF_SIZE, |allocator| {
        let layout = Layout::from_size_align(32, 1).unwrap();
        let p1 = allocator.allocate(layout).unwrap();
        unsafe { allocator.deallocate(p1.cast(), layout) };
        let layout = Layout::from_size_align(40961, 1).unwrap();
        let layout2 = Layout::from_size_align(1381, 1).unwrap();
        let p2 = allocator.allocate(layout).unwrap();
        let p3 = allocator.allocate(layout2).unwrap();
        unsafe { allocator.deallocate(p2.cast(), layout) };
        unsafe { allocator.deallocate(p3.cast(), layout2) };
    });
}

#[test]
fn test_malloc_and_free_gap() {
    // malloc 1 k and 2 k alternately, then consumes remain memory
    fn _test_malloc_and_free_gap(times: usize, heap_size: usize, leaf_size: usize) {
        with_allocator(heap_size, leaf_size, |allocator| {
            let blocks_num = allocator.available_bytes() / leaf_size;

            for _i in 0..times {
                let mut available_bytes = allocator.available_bytes();
                let mut ptrs = Vec::new();
                // align blocks to n times of 4
                for _j in 0..blocks_num / 4 {
                    // alloc 1 k block
                    let bytes = block_size(1, leaf_size) >> 1;
                    let p = allocator.allocate(Layout::from_size_align(bytes, 1).unwrap());
                    assert!(p.is_ok());
                    ptrs.push(p.unwrap());
                    available_bytes -= bytes;
                    // alloc 2 k block
                    let bytes = block_size(2, leaf_size) >> 1;
                    let p = allocator.allocate(Layout::from_size_align(bytes, 1).unwrap());
                    assert!(p.is_ok());
                    ptrs.push(p.unwrap());
                    available_bytes -= bytes;
                }

                for _j in 0..blocks_num / 4 {
                    // alloc 1 k block
                    let bytes = block_size(1, leaf_size) >> 1;
                    let p = allocator.allocate(Layout::from_size_align(bytes, 1).unwrap());
                    assert!(p.is_ok());
                    ptrs.push(p.unwrap());
                    available_bytes -= bytes;
                }
                // calculate remain blocks
                let remain_blocks = blocks_num - blocks_num / 4 * 4;
                assert_eq!(available_bytes, remain_blocks * leaf_size);
                // space is drained
                for _ in 0..remain_blocks {
                    let p = allocator.allocate(Layout::from_size_align(leaf_size, 1).unwrap());
                    assert!(p.is_ok());
                    ptrs.push(p.unwrap());
                }
                assert!(allocator
                    .allocate(Layout::from_size_align(1, 1).unwrap())
                    .is_err());
                // free allocated blocks
                for ptr in ptrs {
                    unsafe {
                        allocator.deallocate(ptr.cast(), Layout::from_size_align(1, 1).unwrap())
                    };
                }
            }
        });
    }

    // test with heaps: 1M, 2M, 4M, 8M
    for i in &[1, 2, 4, 8] {
        _test_malloc_and_free_gap(10, i * HEAP_SIZE, LEAF_SIZE);
    }
}

#[test]
fn test_example_bug() {
    #[allow(clippy::vec_init_then_push)]
    // simulate example bug
    with_allocator(HEAP_SIZE, LEAF_SIZE, |allocator| {
        let mut ptrs = Vec::new();
        ptrs.push(
            allocator
                .allocate(Layout::from_size_align(4, 1).unwrap())
                .unwrap(),
        );
        ptrs.push(
            allocator
                .allocate(Layout::from_size_align(5, 1).unwrap())
                .unwrap(),
        );
        unsafe { allocator.deallocate(ptrs[0].cast(), Layout::from_size_align(1, 1).unwrap()) };
        ptrs.push(
            allocator
                .allocate(Layout::from_size_align(40, 1).unwrap())
                .unwrap(),
        );
        ptrs.push(
            allocator
                .allocate(Layout::from_size_align(48, 1).unwrap())
                .unwrap(),
        );
        ptrs.push(
            allocator
                .allocate(Layout::from_size_align(80, 1).unwrap())
                .unwrap(),
        );
        ptrs.push(
            allocator
                .allocate(Layout::from_size_align(42, 1).unwrap())
                .unwrap(),
        );
        ptrs.push(
            allocator
                .allocate(Layout::from_size_align(13, 1).unwrap())
                .unwrap(),
        );
        ptrs.push(
            allocator
                .allocate(Layout::from_size_align(8, 1).unwrap())
                .unwrap(),
        );
        ptrs.push(
            allocator
                .allocate(Layout::from_size_align(24, 1).unwrap())
                .unwrap(),
        );
        ptrs.push(
            allocator
                .allocate(Layout::from_size_align(16, 1).unwrap())
                .unwrap(),
        );
        ptrs.push(
            allocator
                .allocate(Layout::from_size_align(1024, 1).unwrap())
                .unwrap(),
        );
        ptrs.push(
            allocator
                .allocate(Layout::from_size_align(104, 1).unwrap())
                .unwrap(),
        );
        ptrs.push(
            allocator
                .allocate(Layout::from_size_align(8, 1).unwrap())
                .unwrap(),
        );
        for ptr in ptrs.into_iter().skip(1) {
            unsafe { allocator.deallocate(ptr.cast(), Layout::from_size_align(1, 1).unwrap()) };
        }
    });
}

#[test]
fn test_alignment() {
    let data = [0u8; 4 << 16];
    println!("Buffer data: {:p}", data.as_ptr());
    let allocator = unsafe { BuddyAlloc::new(BuddyAllocParam::new(data.as_ptr(), 4 << 16, 4096)) };
    let p = allocator
        .allocate(Layout::from_size_align(4, 1).unwrap())
        .unwrap();
    println!("Allocated pointer: {:p}", p);
    // FIXME what does it test??
}
