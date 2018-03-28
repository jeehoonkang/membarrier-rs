# Library for memory barrier

[![Build Status](https://travis-ci.org/jeehoonkang/membarrier-rs.svg?branch=master)](https://travis-ci.org/jeehoonkang/membarrier-rs)
[![License](https://img.shields.io/badge/license-MIT%2FApache--2.0-blue.svg)](https://github.com/jeehoonkang/membarrier-rs)
[![Cargo](https://img.shields.io/crates/v/membarrier.svg)](https://crates.io/crates/membarrier)
[![Documentation](https://docs.rs/membarrier/badge.svg)](https://docs.rs/membarrier)

Memory barrier is one of the strongest synchronization primitives in modern relaxed-memory
concurrency. In relaxed-memory concurrency, two threads may have different viewpoint on the
underlying memory system, e.g. thread T1 may have recognized a value V, while T2 does not know
of V at all. This discrepancy is one of the main reasons why concurrent programming is
hard. Memory barrier synchronizes threads in such a way that after memory barriers, threads have
the same viewpoint on the underlying memory system.

Unfortunately, memory barrier is not cheap. Usually, in modern computer systems, there's a
designated memory barrier instruction, e.g. `MFENCE` in x86 and `DMB SY` in ARM, and they may
take more than 100 cycles. Use of memory barrier instruction may be tolerable for several use
cases, e.g. context switching of a few threads, or synchronizing events that happen only once in
the lifetime of a long process. However, sometimes memory barrier is necessary in a fast path,
which significantly degrades the performance.

In order to reduce the synchronization cost of memory barrier, Linux recently introduced the
`sys_membarrier()` system call. Essentially, it performs memory barrier for every thread, and
it's even slower than the ordinary memory barrier instruction. Then what's the benefit? At the
cost of `sys_membarrier()`, other threads may be exempted form issuing a memory barrier
instruction! In other words, by using `sys_membarrier()`, you can optimize fast path at the
performance cost of slow path.

## Usage

By default, we fall back to memory barrier instruction. Turn on the `linux_membarrier` feature
for using the private expedited membarrier in Linux 4.14 or later.

Use this crate as follows:

```rust
extern crate membarrier;

let membarrier = membarrier::Membarrier::new();
membarrier.fast_path();
membarrier.normal_path();
membarrier.slow_path();
```

## Semantics

Formally, there are three kinds of memory barrier: ones for fast path, normal path, and slow
path. In an execution of a program, there is a total order over all instances of memory
barrier. If thread A issues barrier X and thread B issues barrier Y and X is ordered before Y,
then A's knowledge on the underlying memory system at the time of X is transferred to B after Y,
if:

- Either A's or B's barrier is for slow path; or
- Both A's and B's barriers are for normal path or for slow path.

## Reference

For more information, see the [Linux `man` page for
`membarrier`](http://man7.org/linux/man-pages/man2/membarrier.2.html).
