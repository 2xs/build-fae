#![no_std]

extern crate alloc;

use core::alloc::{GlobalAlloc, Layout};
use core::cell::UnsafeCell;
use core::fmt::{self, Write};
use core::hint::spin_loop;
use core::panic::PanicInfo;
use core::ptr::{self, copy_nonoverlapping, null_mut};
use core::sync::atomic::{AtomicBool, Ordering};

#[cfg(target_arch = "arm")]
const SYS_WRITEC: u32 = 0x03;
#[cfg(target_arch = "arm")]
const SYS_READC: u32 = 0x07;
const HEAP_SIZE: usize = 1024;
const MIN_BLOCK_SIZE: usize = core::mem::size_of::<FreeBlock>();

unsafe extern "Rust" {
    fn fae_main() -> i32;
}

#[repr(C)]
struct AllocationHeader {
    total_size: usize,
    user_offset: usize,
}

#[repr(C)]
struct FreeBlock {
    size: usize,
    next: *mut FreeBlock,
}

struct AllocatorState {
    initialized: bool,
    head: *mut FreeBlock,
}

#[repr(align(16))]
struct HeapStorage([u8; HEAP_SIZE]);

struct FirstFitAllocator {
    locked: AtomicBool,
    state: UnsafeCell<AllocatorState>,
    heap: UnsafeCell<HeapStorage>,
}

unsafe impl Sync for FirstFitAllocator {}

#[global_allocator]
static ALLOCATOR: FirstFitAllocator = FirstFitAllocator::new();

impl FirstFitAllocator {
    const fn new() -> Self {
        Self {
            locked: AtomicBool::new(false),
            state: UnsafeCell::new(AllocatorState {
                initialized: false,
                head: null_mut(),
            }),
            heap: UnsafeCell::new(HeapStorage([0; HEAP_SIZE])),
        }
    }

    fn init(&self) {
        self.with_lock(|state, heap| {
            if state.initialized {
                return;
            }

            let block = heap.0.as_mut_ptr() as *mut FreeBlock;
            unsafe {
                ptr::write(
                    block,
                    FreeBlock {
                        size: HEAP_SIZE,
                        next: null_mut(),
                    },
                );
            }
            state.head = block;
            state.initialized = true;
        });
    }

    fn allocate(&self, layout: Layout) -> *mut u8 {
        let size = layout.size().max(1);
        let align = layout.align().max(core::mem::align_of::<AllocationHeader>());

        self.with_lock(|state, _heap| {
            if !state.initialized {
                return null_mut();
            }

            let mut prev: *mut FreeBlock = null_mut();
            let mut current = state.head;

            while !current.is_null() {
                let block_start = current as usize;
                let user_start =
                    align_up(block_start + core::mem::size_of::<AllocationHeader>(), align);
                let total_needed = user_start
                    .checked_add(size)
                    .and_then(|alloc_end| alloc_end.checked_sub(block_start))
                    .unwrap_or(usize::MAX);

                let block_size = unsafe { (*current).size };
                if total_needed <= block_size {
                    let mut consumed = total_needed;
                    let remainder_size = block_size - consumed;
                    let next = unsafe { (*current).next };

                    if remainder_size >= MIN_BLOCK_SIZE {
                        let remainder = (block_start + consumed) as *mut FreeBlock;
                        unsafe {
                            ptr::write(
                                remainder,
                                FreeBlock {
                                    size: remainder_size,
                                    next,
                                },
                            );
                        }
                        if prev.is_null() {
                            state.head = remainder;
                        } else {
                            unsafe { (*prev).next = remainder };
                        }
                    } else {
                        consumed = block_size;
                        if prev.is_null() {
                            state.head = next;
                        } else {
                            unsafe { (*prev).next = next };
                        }
                    }

                    let header = (user_start - core::mem::size_of::<AllocationHeader>())
                        as *mut AllocationHeader;
                    unsafe {
                        ptr::write(
                            header,
                            AllocationHeader {
                                total_size: consumed,
                                user_offset: user_start - block_start,
                            },
                        );
                    }

                    return user_start as *mut u8;
                }

                prev = current;
                current = unsafe { (*current).next };
            }

            null_mut()
        })
    }

    unsafe fn deallocate(&self, ptr: *mut u8) {
        if ptr.is_null() {
            return;
        }

        self.with_lock(|state, _heap| {
            if !state.initialized {
                return;
            }

            let header_ptr =
                ptr.sub(core::mem::size_of::<AllocationHeader>()) as *mut AllocationHeader;
            let header = ptr::read(header_ptr);
            let block_start = ptr.sub(header.user_offset) as *mut FreeBlock;

            ptr::write(
                block_start,
                FreeBlock {
                    size: header.total_size,
                    next: null_mut(),
                },
            );

            insert_free_block(state, block_start);
        });
    }

    fn with_lock<R>(&self, f: impl FnOnce(&mut AllocatorState, &mut HeapStorage) -> R) -> R {
        while self
            .locked
            .compare_exchange(false, true, Ordering::Acquire, Ordering::Relaxed)
            .is_err()
        {
            spin_loop();
        }

        let result = unsafe { f(&mut *self.state.get(), &mut *self.heap.get()) };
        self.locked.store(false, Ordering::Release);
        result
    }
}

unsafe impl GlobalAlloc for FirstFitAllocator {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        self.allocate(layout)
    }

    unsafe fn dealloc(&self, ptr: *mut u8, _layout: Layout) {
        self.deallocate(ptr);
    }
}

fn insert_free_block(state: &mut AllocatorState, block: *mut FreeBlock) {
    let block_addr = block as usize;
    let mut prev: *mut FreeBlock = null_mut();
    let mut current = state.head;

    while !current.is_null() && (current as usize) < block_addr {
        prev = current;
        current = unsafe { (*current).next };
    }

    unsafe {
        (*block).next = current;
    }

    if prev.is_null() {
        state.head = block;
    } else {
        unsafe {
            (*prev).next = block;
        }
    }

    coalesce_next(block);
    if !prev.is_null() {
        coalesce_next(prev);
    }
}

fn coalesce_next(block: *mut FreeBlock) {
    if block.is_null() {
        return;
    }

    unsafe {
        let next = (*block).next;
        if next.is_null() {
            return;
        }

        let block_end = block as usize + (*block).size;
        if block_end == next as usize {
            (*block).size += (*next).size;
            (*block).next = (*next).next;
        }
    }
}

const fn align_up(value: usize, align: usize) -> usize {
    let mask = align - 1;
    (value + mask) & !mask
}

#[unsafe(no_mangle)]
pub extern "C" fn start() -> i32 {
    runtime_init();
    unsafe { fae_main() }
}

pub fn runtime_init() {
    ALLOCATOR.init();
}

#[unsafe(no_mangle)]
pub extern "C" fn __aeabi_unwind_cpp_pr0() {}

#[unsafe(no_mangle)]
pub extern "C" fn __aeabi_unwind_cpp_pr1() {}

#[unsafe(no_mangle)]
pub extern "C" fn abort() -> ! {
    loop {
        spin_loop();
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn memcpy(dest: *mut u8, src: *const u8, len: usize) -> *mut u8 {
    copy_nonoverlapping(src, dest, len);
    dest
}

pub fn write_buffer(buf: &[u8]) {
    for &byte in buf {
        semihost_writec(byte);
    }
}

pub fn write_error_buffer(buf: &[u8]) {
    for &byte in buf {
        semihost_writec(byte);
    }
}

pub fn _print(args: fmt::Arguments<'_>) {
    let mut console = Console;
    let _ = console.write_fmt(args);
}

pub fn _eprint(args: fmt::Arguments<'_>) {
    let mut console = ErrorConsole;
    let _ = console.write_fmt(args);
}

pub fn read_char() -> u8 {
    semihost_readc()
}

pub fn read_buffer(buf: &mut [u8]) -> usize {
    for (i, slot) in buf.iter_mut().enumerate() {
        *slot = read_char();
        if *slot == b'\n' || *slot == b'\r' {
            return i + 1;
        }
    }
    buf.len()
}

pub struct Console;
pub struct ErrorConsole;

impl Write for Console {
    fn write_str(&mut self, s: &str) -> fmt::Result {
        write_buffer(s.as_bytes());
        Ok(())
    }
}

impl Write for ErrorConsole {
    fn write_str(&mut self, s: &str) -> fmt::Result {
        write_error_buffer(s.as_bytes());
        Ok(())
    }
}

#[cfg(target_arch = "arm")]
fn semihost_writec(byte: u8) {
    let mut ch = byte;
    unsafe {
        core::arch::asm!(
            "mov r0, {sys_writec}",
            "mov r1, {arg}",
            "bkpt 0xab",
            sys_writec = in(reg) SYS_WRITEC,
            arg = in(reg) &mut ch,
            out("r0") _,
            out("r1") _,
            options(nostack)
        );
    }
}

#[cfg(not(target_arch = "arm"))]
fn semihost_writec(_byte: u8) {}

#[cfg(target_arch = "arm")]
fn semihost_readc() -> u8 {
    let value: u32;
    unsafe {
        core::arch::asm!(
            "mov r0, {sys_readc}",
            "mov r1, #0",
            "bkpt 0xab",
            sys_readc = in(reg) SYS_READC,
            lateout("r0") value,
            out("r1") _,
            options(nostack)
        );
    }
    value as u8
}

#[cfg(not(target_arch = "arm"))]
fn semihost_readc() -> u8 {
    0
}

#[panic_handler]
fn panic(info: &PanicInfo<'_>) -> ! {
    _eprint(format_args!("panic: {info}\n"));
    loop {
        spin_loop();
    }
}

#[macro_export]
macro_rules! entry {
    ($path:path) => {
        #[used]
        static __FAE_KEEP_START: extern "C" fn() -> i32 = $crate::start;

        #[unsafe(no_mangle)]
        fn fae_main() -> i32 {
            $path()
        }
    };
}

#[macro_export]
macro_rules! print {
    ($($arg:tt)*) => {
        $crate::_print(core::format_args!($($arg)*))
    };
}

#[macro_export]
macro_rules! eprint {
    ($($arg:tt)*) => {
        $crate::_eprint(core::format_args!($($arg)*))
    };
}

#[macro_export]
macro_rules! println {
    () => {
        $crate::print!("\n")
    };
    ($fmt:expr) => {
        $crate::print!(core::concat!($fmt, "\n"))
    };
    ($fmt:expr, $($arg:tt)*) => {
        $crate::print!(core::concat!($fmt, "\n"), $($arg)*)
    };
}

#[macro_export]
macro_rules! eprintln {
    () => {
        $crate::eprint!("\n")
    };
    ($fmt:expr) => {
        $crate::eprint!(core::concat!($fmt, "\n"))
    };
    ($fmt:expr, $($arg:tt)*) => {
        $crate::eprint!(core::concat!($fmt, "\n"), $($arg)*)
    };
}
