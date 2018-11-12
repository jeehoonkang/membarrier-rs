//! Library for memory barrier.
//!
//! Memory barrier is one of the strongest synchronization primitives in modern relaxed-memory
//! concurrency. In relaxed-memory concurrency, two threads may have different viewpoint on the
//! underlying memory system, e.g. thread T1 may have recognized a value V, while T2 does not know
//! of V at all. This discrepancy is one of the main reasons why concurrent programming is
//! hard. Memory barrier synchronizes threads in such a way that after memory barriers, threads have
//! the same viewpoint on the underlying memory system.
//!
//! Unfortunately, memory barrier is not cheap. Usually, in modern computer systems, there's a
//! designated memory barrier instruction, e.g. `MFENCE` in x86 and `DMB SY` in ARM, and they may
//! take more than 100 cycles. Use of memory barrier instruction may be tolerable for several use
//! cases, e.g. context switching of a few threads, or synchronizing events that happen only once in
//! the lifetime of a long process. However, sometimes memory barrier is necessary in a fast path,
//! which significantly degrades the performance.
//!
//! In order to reduce the synchronization cost of memory barrier, Linux recently introduced the
//! `sys_membarrier()` system call. Essentially, it performs memory barrier for every thread, and
//! it's even slower than the ordinary memory barrier instruction. Then what's the benefit? At the
//! cost of `sys_membarrier()`, other threads may be exempted form issuing a memory barrier
//! instruction! In other words, by using `sys_membarrier()`, you can optimize fast path at the
//! performance cost of slow path.
//!
//! # Usage
//!
//! By default, we fall back to memory barrier instruction. Turn on the `linux_membarrier` feature
//! for using the private expedited membarrier in Linux 4.14 or later.
//!
//! Use this crate as follows:
//!
//! ```
//! extern crate membarrier;
//!
//! let membarrier = membarrier::Membarrier::new();
//! membarrier.fast_path();
//! membarrier.normal_path();
//! membarrier.slow_path();
//! ```
//!
//! # Semantics
//!
//! Formally, there are three kinds of memory barrier: ones for fast path, normal path, and slow
//! path. In an execution of a program, there is a total order over all instances of memory
//! barrier. If thread A issues barrier X and thread B issues barrier Y and X is ordered before Y,
//! then A's knowledge on the underlying memory system at the time of X is transferred to B after Y,
//! if:
//!
//! - Either A's or B's barrier is for slow path; or
//! - Both A's and B's barriers are for normal path or for slow path.
//!
//! # Reference
//!
//! For more information, see the [Linux `man` page for
//! `membarrier`](http://man7.org/linux/man-pages/man2/membarrier.2.html).

#![warn(missing_docs, missing_debug_implementations)]

extern crate core;
#[macro_use]
extern crate cfg_if;
#[allow(unused_imports)]
#[macro_use]
extern crate lazy_static;
extern crate kernel32;
extern crate libc;

cfg_if! {
    if #[cfg(all(target_os = "linux", feature = "linux_membarrier"))] {
        pub use linux_membarrier::Membarrier;
    } else if #[cfg(target_os = "windows")] {
        pub use windows_membarrier::Membarrier;
    } else {
        pub use default::Membarrier;
    }
}

impl Default for Membarrier {
    fn default() -> Self {
        Self::new()
    }
}

mod default {
    use core::sync::atomic;

    /// The default membarrier manager.
    ///
    /// It issues memory barrier instruction for fast, normal, and slow path.
    #[derive(Debug, Clone, Copy)]
    pub struct Membarrier {}

    impl Membarrier {
        /// Creates a membarrier manager.
        #[inline]
        pub fn new() -> Self {
            Self {}
        }

        /// Issues memory barrier for fast path.
        ///
        /// It just issues the memory barrier instruction.
        #[inline]
        pub fn fast_path(self) {
            atomic::fence(atomic::Ordering::SeqCst);
        }

        /// Issues memory barrier for normal path.
        ///
        /// It just issues the memory barrier instruction.
        #[inline]
        pub fn normal_path(self) {
            atomic::fence(atomic::Ordering::SeqCst);
        }

        /// Issues memory barrier for slow path.
        ///
        /// It just issues the memory barrier instruction.
        #[inline]
        pub fn slow_path(self) {
            atomic::fence(atomic::Ordering::SeqCst);
        }
    }
}

#[cfg(target_os = "linux")]
mod linux_membarrier {
    use core::sync::atomic;
    use libc;

    /// Commands for the membarrier system call.
    ///
    /// # Caveat
    ///
    /// We're defining it here because, unfortunately, the `libc` crate currently doesn't expose
    /// `membarrier_cm` for us. You can find the numbers in the [Linux source
    /// code](https://github.com/torvalds/linux/blob/master/include/uapi/linux/membarrier.h).
    ///
    /// This enum should really be `#[repr(libc::c_int)]`, but Rust currently doesn't allow it.
    #[repr(i32)]
    #[allow(dead_code, non_camel_case_types)]
    enum membarrier_cmd {
        MEMBARRIER_CMD_QUERY = 0,
        MEMBARRIER_CMD_GLOBAL = (1 << 0),
        MEMBARRIER_CMD_GLOBAL_EXPEDITED = (1 << 1),
        MEMBARRIER_CMD_REGISTER_GLOBAL_EXPEDITED = (1 << 2),
        MEMBARRIER_CMD_PRIVATE_EXPEDITED = (1 << 3),
        MEMBARRIER_CMD_REGISTER_PRIVATE_EXPEDITED = (1 << 4),
        MEMBARRIER_CMD_PRIVATE_EXPEDITED_SYNC_CORE = (1 << 5),
        MEMBARRIER_CMD_REGISTER_PRIVATE_EXPEDITED_SYNC_CORE = (1 << 6),
    }

    /// Call the `sys_membarrier` system call.
    #[inline]
    fn membarrier(cmd: membarrier_cmd) -> libc::c_long {
        unsafe { libc::syscall(libc::SYS_membarrier, cmd as libc::c_int, 0 as libc::c_int) }
    }

    lazy_static! {
        /// Represents whether the `sys_membarrier` system call is supported.
        static ref IS_SUPPORTED: bool = {
            // Queries which membarrier commands are supported. Checks if private expedited
            // membarrier is supported.
            let ret = membarrier(membarrier_cmd::MEMBARRIER_CMD_QUERY);
            if ret < 0 ||
                ret & membarrier_cmd::MEMBARRIER_CMD_PRIVATE_EXPEDITED as libc::c_long == 0 ||
                ret & membarrier_cmd::MEMBARRIER_CMD_REGISTER_PRIVATE_EXPEDITED as libc::c_long == 0
            {
                return false;
            }

            // Registers the current process as a user of private expedited membarrier.
            if membarrier(membarrier_cmd::MEMBARRIER_CMD_REGISTER_PRIVATE_EXPEDITED) < 0 {
                return false;
            }

            true
        };
    }

    /// The membarrier manager based on Linux's `sys_membarrier()` system call.
    ///
    /// Its existence guarantees that it is ready to call private expedited membarrier in the
    /// current process.
    ///
    /// For fast path, it issues compiler fence, which is basically zero-cost. For normal path, it
    /// issues memory barrier instruction. For slow path, it calls the `sys_membarrier()` system
    /// call.
    #[derive(Debug, Clone, Copy)]
    pub struct Membarrier {}

    impl Membarrier {
        /// Creates a membarrier manager.
        #[inline]
        #[allow(dead_code)]
        pub fn new() -> Self {
            assert!(*IS_SUPPORTED, "linux membarrier is not supported");
            Self {}
        }

        /// Issues memory barrier for fast path.
        ///
        /// It issues compiler fence, which disallows compiler optimizations across itself.
        #[inline]
        #[allow(dead_code)]
        pub fn fast_path(self) {
            atomic::compiler_fence(atomic::Ordering::SeqCst);
        }

        /// Issues memory barrier for normal path.
        ///
        /// It just issues the memory barrier instruction.
        #[inline]
        #[allow(dead_code)]
        pub fn normal_path(self) {
            atomic::fence(atomic::Ordering::SeqCst);
        }

        /// Issues memory barrier for slow path.
        ///
        /// It issues private expedited membarrier using the `sys_membarrier()` system call.
        #[inline]
        #[allow(dead_code)]
        pub fn slow_path(self) {
            if membarrier(membarrier_cmd::MEMBARRIER_CMD_PRIVATE_EXPEDITED) < 0 {
                panic!("membarrier(membarrier_cmd_private_expedited) failed");
            }
        }
    }
}

#[cfg(target_os = "windows")]
mod windows_membarrier {
    use core::sync::atomic;
    use kernel32;

    /// The membarrier manager based on Windows's `FlushProcessWriteBuffers()` system call.
    ///
    /// For fast path, it issues compiler fence, which is basically zero-cost. For normal path, it
    /// issues memory barrier instruction. For slow path, it calls the `FlushProcessWriteBuffers()`
    /// system call.
    #[derive(Debug, Clone, Copy)]
    pub struct Membarrier {}

    impl Membarrier {
        /// Creates a membarrier manager.
        #[inline]
        pub fn new() -> Self {
            Self {}
        }

        /// Issues memory barrier for fast path.
        ///
        /// It issues compiler fence, which disallows compiler optimizations across itself.
        #[inline]
        pub fn fast_path(self) {
            atomic::compiler_fence(atomic::Ordering::SeqCst);
        }

        /// Issues memory barrier for normal path.
        ///
        /// It just issues the memory barrier instruction.
        #[inline]
        pub fn normal_path(self) {
            atomic::fence(atomic::Ordering::SeqCst);
        }

        /// Issues memory barrier for slow path.
        ///
        /// It invokes the `FlushProcessWriteBuffers()` system call.
        #[inline]
        pub fn slow_path(self) {
            unsafe {
                kernel32::FlushProcessWriteBuffers();
            }
        }
    }
}
