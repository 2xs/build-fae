pub const XIPFS_EXEC_ARGC_MAX: usize = 64;

#[repr(C)]
pub struct XipfsRt0CtxData {
    pub file_base: *mut u8,

    pub is_safe_call: u8,

    pub syscall_table: *mut *mut u8,

    pub argc: u32,

    pub argv: [*mut u8; XIPFS_EXEC_ARGC_MAX],

    pub former_got: *const u8,

    pub current_got: *const u8,
}
