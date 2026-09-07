#[repr(C)]
pub struct XipfsRt0Ctx {
    /// Start address of the binary in the NVM
    pub bin_base: *mut u8,
    /// Start address of the available free RAM
    pub ram_start: *mut u8,
    /// End address of the available free RAM
    pub ram_end: *mut u8,
    /// Start address of the free NVM
    pub nvm_start: *mut u8,
    /// End address of the free NVM
    pub nvm_end: *mut u8,
    /// Pointer to loader-specific context data.
    /// For xipfs, this points to a `XipfsRt0CtxData`
    pub argv: *mut u8,
}
