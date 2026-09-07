use core::{
    arch::asm,
    ffi::{c_char, CStr},
};

use crate::{
    xipfs_rt0_ctx::XipfsRt0Ctx,
    xipfs_rt0_ctx_data::{XipfsRt0CtxData, XIPFS_EXEC_ARGC_MAX},
    xipfs_syscall::{
        XipfsSyscall, XipfsSyscallCopyFile, XipfsSyscallExit, XipfsSyscallGetFileSize,
        XipfsSyscallGetLed, XipfsSyscallGetTemp, XipfsSyscallIsPrint, XipfsSyscallPrintf,
        XipfsSyscallSetLed, XipfsSyscallStrTol,
    },
};

#[doc(hidden)]
pub use core::fmt::Write as __Write;

#[doc(hidden)]
pub use crate::cstr_buf::CStrBuf as __CStrBuf;

#[macro_export]

/// Print a Rust formatted string through the RIOT syscall
macro_rules! print {
    ($($arg:tt)*) => {{
        let mut buf = [0u8; 128];
        let mut w = $crate::stdriot::__CStrBuf::new(&mut buf);
        // ignore the Result : if formatting overflows, output is truncated
        let _ = <$crate::stdriot::__CStrBuf as $crate::stdriot::__Write>::write_fmt(&mut w, format_args!($($arg)*));
        if let Ok(s) = w.add_null_terminator() {
            $crate::stdriot::printf(s);
        }
    }};
}

/// Create a &CStr from a &str
macro_rules! to_cstr {
    ($buf:ident, $s:expr) => {{
        let mut w = $crate::stdriot::__CStrBuf::new(&mut $buf);
        match write!(w, "{}", $s) {
            Ok(_) => match w.add_null_terminator() {
                Ok(c) => c,
                Err(_) => c"Error: add_null_terminator failed",
            },
            Err(_) => c"Error: write! failed",
        }
    }};
}

unsafe extern "C" {
    pub unsafe fn main() -> i32;
}

static mut FORMER_GOT: *const u8 = core::ptr::null_mut();
static mut CURRENT_GOT: *const u8 = core::ptr::null_mut();
static mut XIPFS_SYSCALL_TABLE: *mut *mut u8 = core::ptr::null_mut();
// Must be called with sl = RIOT's GOT before any SVC-based syscall wrapper.
// r10/sl is RIOT's static base; the payload uses r9 instead.
#[inline(always)]
unsafe fn set_sl(ptr: *const u8) {
    asm!(
        "mov r10, {0}",
        in(reg) ptr,
        out("r10") _,
        options(nostack, nomem, preserves_flags)
    );
}

/// Take a XipfsSyscall enum that designates the syscall to be used.
/// Return a raw pointer pointing at the correct RIOT syscall
pub unsafe fn syscall_ptr(syscall: XipfsSyscall) -> *mut u8 {
    let xipfs_syscall_table = XIPFS_SYSCALL_TABLE;
    *xipfs_syscall_table.add(syscall as usize)
}

/// Call the exit RIOT syscall with the provided status
/// Normally should not reach the end as it calls RIOT exit
pub unsafe fn exit(status: i32) {
    let raw_ptr: *mut u8 = syscall_ptr(XipfsSyscall::Exit);
    let exit_fn: XipfsSyscallExit = core::mem::transmute(raw_ptr);
    set_sl(FORMER_GOT);
    exit_fn(status);
    set_sl(CURRENT_GOT);
}

#[doc(hidden)]
pub unsafe fn printf(format: &CStr) -> i32 {
    let raw_ptr: *mut u8 = syscall_ptr(XipfsSyscall::Vprintf);
    let print_fn: XipfsSyscallPrintf = core::mem::transmute(raw_ptr);
    set_sl(FORMER_GOT);
    let res: i32 = print_fn(format.as_ptr(), 0usize);
    set_sl(CURRENT_GOT);
    res
}

/// Call the isprint RIOT syscall with the provided character.
/// Return an integer that indicates whether the character is printable if > 0 or not.
pub unsafe fn isprint(character: u8) -> i32 {
    let raw_ptr: *mut u8 = syscall_ptr(XipfsSyscall::IsPrint);
    let isprint_fn: XipfsSyscallIsPrint = core::mem::transmute(raw_ptr);
    set_sl(FORMER_GOT);
    let res: i32 = isprint_fn(character);
    set_sl(CURRENT_GOT);
    res
}

/// Call the strtol RIOT syscall with the provided &str and a base.
/// Return an integer corresponding to the translated number.
/// TODO : maybe somehow return end_ptr ?
pub unsafe fn strtol(string: &str, base: i32) -> i32 {
    let mut buf = [0u8; 64];
    let c_string = to_cstr!(buf, string);
    let raw_ptr: *mut u8 = syscall_ptr(XipfsSyscall::Strtol);
    let strtol_fn: XipfsSyscallStrTol = core::mem::transmute(raw_ptr);

    let mut end_ptr: *mut c_char = core::ptr::null_mut();
    set_sl(FORMER_GOT);
    let res = strtol_fn(c_string.as_ptr(), &raw mut end_ptr, base);
    set_sl(CURRENT_GOT);
    res
}

/// Call the get_led RIOT syscall with the provided pos.
/// Return the status of the corresponding led.
pub unsafe fn get_led(pos: i32) -> i32 {
    let raw_ptr: *mut u8 = syscall_ptr(XipfsSyscall::GetLed);
    let getled_fn: XipfsSyscallGetLed = core::mem::transmute(raw_ptr);
    set_sl(FORMER_GOT);
    let res = getled_fn(pos);
    set_sl(CURRENT_GOT);
    res
}

/// Call the set_led RIOT syscall with the provided pos and val.
/// Set the corresponding led to the either on or off depending on val.
pub unsafe fn set_led(pos: i32, val: i32) -> i32 {
    let raw_ptr: *mut u8 = syscall_ptr(XipfsSyscall::SetLed);
    let getled_fn: XipfsSyscallSetLed = core::mem::transmute(raw_ptr);
    set_sl(FORMER_GOT);
    let res = getled_fn(pos, val);
    set_sl(CURRENT_GOT);
    res
}

/// TODO : Test
pub unsafe fn copy_file(name: &CStr, buf: *mut u8, nbyte: usize) -> bool {
    let raw_ptr: *mut u8 = syscall_ptr(XipfsSyscall::CopyFile);
    let copyfile_fn: XipfsSyscallCopyFile = core::mem::transmute(raw_ptr);
    set_sl(FORMER_GOT);
    let res = copyfile_fn(name.as_ptr(), buf, nbyte);
    set_sl(CURRENT_GOT);
    res
}

/// Call the get_file_size RIOT syscall with the pathname of the file.
/// Return an integer indicating if the file exists, and if so, write the size in the size parameter
pub unsafe fn get_file_size(name: &str, size: *mut usize) -> i32 {
    let mut buf = [0u8; 64];
    let c_name = to_cstr!(buf, name);
    printf(c_name);
    let raw_ptr: *mut u8 = syscall_ptr(XipfsSyscall::GetFileSize);
    let getfilesize_fn: XipfsSyscallGetFileSize = core::mem::transmute(raw_ptr);
    set_sl(FORMER_GOT);
    let res = getfilesize_fn(c_name.as_ptr(), size);
    set_sl(CURRENT_GOT);
    res
}

/// Call the get_temp RIOT syscall
/// Return the temperature of the board
pub unsafe fn get_temp() -> i32 {
    let res: i32;
    let raw_ptr: *mut u8 = syscall_ptr(XipfsSyscall::GetTemp);
    let get_temp_fn: XipfsSyscallGetTemp = core::mem::transmute(raw_ptr);
    set_sl(FORMER_GOT);
    res = get_temp_fn();
    set_sl(CURRENT_GOT);
    res
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn start(xipfs_rt0_ctx: *const XipfsRt0Ctx) {
    let ctx = &*xipfs_rt0_ctx;
    let ctx_data = &*(ctx.argv as *const XipfsRt0CtxData);
    FORMER_GOT = ctx_data.former_got;
    CURRENT_GOT = ctx_data.current_got;
    XIPFS_SYSCALL_TABLE = ctx_data.syscall_table;
    let argc = (ctx_data.argc as usize).min(XIPFS_EXEC_ARGC_MAX);
    let _argv = &ctx_data.argv[..argc];
    let status = main();

    exit(status);
}
