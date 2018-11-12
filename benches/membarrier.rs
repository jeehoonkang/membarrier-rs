#![feature(test)]

extern crate membarrier;
extern crate test;

use membarrier::Membarrier;
use test::Bencher;

#[bench]
fn fast_path(b: &mut Bencher) {
    let membarrier = Membarrier::new();
    b.iter(|| {
        membarrier.fast_path();
    });
}

#[bench]
fn normal_path(b: &mut Bencher) {
    let membarrier = Membarrier::new();
    b.iter(|| {
        membarrier.normal_path();
    });
}

#[bench]
fn slow_path(b: &mut Bencher) {
    let membarrier = Membarrier::new();
    b.iter(|| {
        membarrier.slow_path();
    });
}
