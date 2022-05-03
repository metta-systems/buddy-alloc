#![cfg_attr(not(test), no_std)]

pub mod buddy_alloc;
pub mod freelist_alloc;
pub mod non_threadsafe_alloc;
#[cfg(test)]
mod tests;

pub use crate::{
    buddy_alloc::{BuddyAlloc, BuddyAllocParam},
    freelist_alloc::{FreelistAlloc, FreelistAllocParam},
    non_threadsafe_alloc::NonThreadsafeAlloc,
};
