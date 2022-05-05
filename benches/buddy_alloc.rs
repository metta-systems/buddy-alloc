#![feature(allocator_api)]

#[macro_use]
extern crate criterion;

use {
    criterion::{Criterion, Throughput},
    std::alloc::{Allocator, Layout},
};

use buddy_alloc::buddy_alloc::{BuddyAlloc, BuddyAllocParam};

const HEAP_SIZE: usize = 64 * 1024 * 1024; // 64 MB
const ALLOC_SIZE: usize = 32 * 1024 * 1024;
const LEAF_SIZE: usize = 16;

fn with_allocator<F: FnOnce(BuddyAlloc)>(f: F) {
    let buf: Vec<u8> = Vec::with_capacity(HEAP_SIZE);
    unsafe {
        let param = BuddyAllocParam::new(buf.as_ptr(), HEAP_SIZE, LEAF_SIZE);
        let allocator = BuddyAlloc::new(param);
        f(allocator);
    }
}

fn bench_alloc(allocator: &mut BuddyAlloc, alloc_size: usize) {
    for _i in 0..(ALLOC_SIZE / alloc_size) {
        allocator
            .allocate(Layout::from_size_align(alloc_size, 1).unwrap())
            .unwrap();
    }
}

fn bench_alloc_then_free(allocator: &mut BuddyAlloc, alloc_size: usize) {
    let count = ALLOC_SIZE / alloc_size;
    let layout = Layout::from_size_align(alloc_size, 1).unwrap();
    let mut ptrs = Vec::with_capacity(count);
    for _i in 0..count {
        ptrs.push(allocator.allocate(layout).unwrap());
    }
    for _i in 0..count {
        unsafe { allocator.deallocate(ptrs.pop().unwrap().cast(), layout) };
    }
}

fn bench(c: &mut Criterion) {
    let mut group = c.benchmark_group("alloc");
    for &size in &[16, 32, 64, 128] {
        let count = ALLOC_SIZE / size;
        group.throughput(Throughput::Elements(count as u64));
        group.bench_with_input(format!("{} Bytes", size), &size, |b, &size| {
            with_allocator(|mut allocator| b.iter(|| bench_alloc(&mut allocator, size)));
        });
    }
    group.finish();

    let mut group = c.benchmark_group("alloc then free");
    for &size in &[16, 32, 64, 128] {
        let count = ALLOC_SIZE / size;
        group.throughput(Throughput::Elements(count as u64));
        group.bench_with_input(format!("{} Bytes", size), &size, |b, &size| {
            with_allocator(|mut allocator| b.iter(|| bench_alloc_then_free(&mut allocator, size)));
        });
    }
    group.finish();
}

criterion_group!(
    name = benches;
    config = Criterion::default().sample_size(20);
    targets = bench
);
criterion_main!(benches);
