#![no_std]

extern crate membarrier;

use core::sync::atomic::{fence, Ordering};

#[test]
fn fences() {
    membarrier::light();     // light-weight barrier
    fence(Ordering::SeqCst); // normal barrier
    membarrier::heavy();     // heavy-weight barrier
}
