use core::ffi::c_char;

#[repr(C)]
pub enum XipfsSyscall {
    Exit,
    Vprintf,
    GetTemp,
    IsPrint,
    Strtol,
    GetLed,
    SetLed,
    CopyFile,
    GetFileSize,
    Memset,
    Max,
}

pub type XipfsSyscallExit = unsafe extern "C" fn(status: i32) -> i32;
// Matches C: int vprintf(const char *format, va_list ap)
// va_list is a pointer; pass null when format has no specifiers.
pub type XipfsSyscallPrintf = unsafe extern "C" fn(format: *const c_char, ap: usize) -> i32;
pub type XipfsSyscallGetTemp = unsafe extern "C" fn() -> i32;
pub type XipfsSyscallIsPrint = unsafe extern "C" fn(character: c_char) -> i32;
pub type XipfsSyscallStrTol =
    unsafe extern "C" fn(string: *const c_char, endptr: *mut *mut c_char, base: i32) -> i32;
pub type XipfsSyscallGetLed = unsafe extern "C" fn(pos: i32) -> i32;
pub type XipfsSyscallSetLed = unsafe extern "C" fn(pos: i32, val: i32) -> i32;
pub type XipfsSyscallCopyFile =
    unsafe extern "C" fn(name: *const c_char, buf: *mut u8, nbyte: usize) -> bool;
pub type XipfsSyscallGetFileSize =
    unsafe extern "C" fn(name: *const c_char, size: *mut usize) -> i32;
