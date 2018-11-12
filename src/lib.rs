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
#![no_std]

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

macro_rules! fatal_assert {
    ($cond:expr) => {
        if !$cond {
            #[allow(unused_unsafe)]
            unsafe {
                libc::abort();
            }
        }
    };
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
    use core::cell::UnsafeCell;
    use core::mem;
    use core::ptr;
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
        static ref MEMBARRIER_IS_SUPPORTED: bool = {
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

    struct MprotectBarrier {
        lock: UnsafeCell<libc::pthread_mutex_t>,
        page: *mut libc::c_void,
        page_size: libc::size_t,
    }

    unsafe impl Sync for MprotectBarrier {}

    impl MprotectBarrier {
        /// Issues a process-wide barrier.
        #[inline]
        fn barrier(&self) {
            unsafe {
                // Lock the mutex.
                fatal_assert!(libc::pthread_mutex_lock(self.lock.get()) == 0);

                // Set the page access protections to read + write.
                fatal_assert!(
                    libc::mprotect(
                        self.page,
                        self.page_size,
                        libc::PROT_READ | libc::PROT_WRITE,
                    ) == 0
                );

                // Ensure that the page is dirty before we change the protection so that we prevent
                // the OS from skipping the global TLB flush.
                let atomic_usize = &*(self.page as *const atomic::AtomicUsize);
                atomic_usize.fetch_add(1, atomic::Ordering::SeqCst);

                // Set the page access protections to none.
                //
                // Changing a page protection from read + write to none causes the OS to issue an
                // interrupt to flush TLBs on all processors. This also results in flushing the
                // processor buffers.
                fatal_assert!(libc::mprotect(self.page, self.page_size, libc::PROT_NONE) == 0);

                // Unlock the mutex.
                fatal_assert!(libc::pthread_mutex_unlock(self.lock.get()) == 0);
            }
        }
    }

    lazy_static! {
        /// An alternative solution to `sys_membarrier` that works on older Linux kernels.
        static ref MPROTECT_BARRIER: MprotectBarrier = {
            unsafe {
                // Find out the page size on the current system.
                let page_size = libc::sysconf(libc::_SC_PAGESIZE);
                fatal_assert!(page_size > 0);
                let page_size = page_size as libc::size_t;

                // Create a dummy page.
                let page = libc::mmap(
                    ptr::null_mut(),
                    page_size,
                    libc::PROT_NONE,
                    libc::MAP_PRIVATE | libc::MAP_ANONYMOUS,
                    -1 as libc::c_int,
                    0 as libc::off_t,
                );
                fatal_assert!(page != libc::MAP_FAILED);
                fatal_assert!(page as libc::size_t % page_size == 0);

                // Locking the page ensures that it stays in memory during the two mprotect calls
                // in `MprotectBarrier::barrier()`. If the page was unmapped between those calls,
                // they would not have the expected effect of generating IPI.
                libc::mlock(page, page_size as libc::size_t);

                // Initialize the mutex.
                let lock = UnsafeCell::new(libc::PTHREAD_MUTEX_INITIALIZER);
                let mut attr: libc::pthread_mutexattr_t = mem::uninitialized();
                fatal_assert!(libc::pthread_mutexattr_init(&mut attr) == 0);
                fatal_assert!(
                    libc::pthread_mutexattr_settype(&mut attr, libc::PTHREAD_MUTEX_NORMAL) == 0
                );
                fatal_assert!(libc::pthread_mutex_init(lock.get(), &attr) == 0);
                fatal_assert!(libc::pthread_mutexattr_destroy(&mut attr) == 0);

                MprotectBarrier { lock, page, page_size }
            }
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
    ///
    /// If `sys_membarrier()` is not supported, then process-wide memory barriers will be issued by
    /// changing access protections of a single mmap-ed page. This method is not as fast as the
    /// `sys_membarrier()` call, but works very similarly.
    #[derive(Debug, Clone, Copy)]
    pub struct Membarrier {}

    impl Membarrier {
        /// Creates a membarrier manager.
        #[inline]
        #[allow(dead_code)]
        pub fn new() -> Self {
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
            if *MEMBARRIER_IS_SUPPORTED {
                fatal_assert!(membarrier(membarrier_cmd::MEMBARRIER_CMD_PRIVATE_EXPEDITED) >= 0);
            } else {
                MPROTECT_BARRIER.barrier();
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
