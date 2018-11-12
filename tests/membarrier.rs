extern crate membarrier;

use membarrier::Membarrier;

#[test]
fn fences() {
    let membarrier = Membarrier::new();

    membarrier.fast_path();
    membarrier.normal_path();
    membarrier.slow_path();
}
