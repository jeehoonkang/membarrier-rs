#![feature(test)]

extern crate test;
extern crate membarrier;

use test::Bencher;
use std::sync::atomic::{fence, Ordering};

#[bench]
fn light(b: &mut Bencher) {
    b.iter(|| {
        membarrier::light();
    });
}

#[bench]
fn normal(b: &mut Bencher) {
    b.iter(|| {
        fence(Ordering::SeqCst);
    });
}

#[bench]
fn heavy(b: &mut Bencher) {
    b.iter(|| {
        membarrier::heavy();
    });
}
