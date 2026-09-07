# FAE Runtime Environment

This document specifies the runtime services exposed to FAE programs by the host environment.

It is not:

- the FAE binary file format specification
- the builder contract for application ELFs
- the implementation details of the historical C runtime

It is the ABI-level contract between:

- the host that executes a FAE program
- the low-level startup code
- the language runtime layer (`start`)
- the application code itself

For execution layering terminology, see [`developer-guide.md`](developer-guide.md).

## Purpose

The FAE runtime environment is intentionally split into small syscall families.

The goal is:

- to support both C and Rust runtimes
- to keep the core ABI very small
- to expose only stable primitives at the syscall-table level
- to let C and Rust build higher-level libraries above those primitives

This document currently specifies:

- `core` syscalls
- `fs` syscalls
- `network` family scope
- `sensors` family scope

Threading is intentionally left as `TODO`.

## Design Principles

### Minimal Host ABI

The syscall table should expose only primitives that are hard or impossible to implement inside a FAE program itself.

Examples:

- program termination
- console I/O
- RAM allocation from host-managed memory
- memory layout discovery
- filesystem operations through the mounted XiPFS instance

### Runtime-Oriented, Not libc-Oriented

The syscall table is not meant to be a copy of libc.

Instead:

- the syscall table is the low-level host ABI
- the C runtime wraps it into libc-like functions
- the Rust runtime wraps it into Rust-friendly APIs and allocators

### Stable Opaque Handles

Where host-owned objects are involved, the syscall ABI should expose opaque handles rather than internal implementation structures.

This is especially important for filesystem descriptors.

## Runtime Families

The runtime environment is currently divided into these families:

- `core`: always available, very small, language-runtime-critical
- `fs`: minimal filesystem access, intended to wrap XiPFS
- `network`: network communication primitives and host-backed sockets/transports
- `sensors`: access to host-exposed sensors and sampled measurements
- `thread`: reserved for future work

## Common ABI Conventions

Unless explicitly stated otherwise:

- integer types are fixed-width where possible
- pointers are valid only in the current process/FAE execution context
- functions return negative values or error codes according to the family-specific contract
- opaque handles are host-owned and must not be dereferenced by the application

Recommended type aliases for the specification:

```c
typedef uint32_t fae_u32;
typedef int32_t fae_i32;
typedef uint32_t fae_size;
typedef int32_t fae_ssize;
typedef uint32_t fae_off;
typedef uint32_t fae_mode;
typedef uint32_t fae_flags;
typedef void *fae_handle_t;
```

Notes:

- `fae_handle_t` is an opaque token managed by the host
- concrete language bindings may choose a stronger wrapper type
- `fae_off` is specified as a 32-bit offset for now, consistent with current XiPFS practice

Additional memory-layout-specific aliases:

```c
typedef uintptr_t fae_addr;
typedef uint32_t fae_mem_type;
typedef uint32_t fae_mem_rights;
typedef uint32_t fae_mem_sharing;
```

## 1. Core Syscalls

The `core` family is the smallest mandatory family.

It is intended to be sufficient for:

- a minimal C runtime
- a minimal Rust runtime with allocation support
- panic/error reporting through the console

### 1.1 `exit`

Terminates the current FAE program.

Prototype:

```c
void fae_core_exit(fae_i32 status);
```

Semantics:

- terminates execution of the current FAE program
- does not return
- `status == 0` means normal termination
- non-zero status values indicate failure or runtime-defined exit categories

### 1.2 `console_write`

Writes raw bytes to the execution console.

Prototype:

```c
fae_ssize fae_core_console_write(const void *buf, fae_size len);
```

Semantics:

- writes up to `len` bytes from `buf`
- returns the number of bytes written
- may return a negative error code on failure
- no formatting is implied
- no terminating `'\0'` is required

Intended use:

- debug logging
- panic output
- basic `stdout`/`stderr` style wrappers

### 1.3 `console_read`

Reads raw bytes from the execution console.

Prototype:

```c
fae_ssize fae_core_console_read(void *buf, fae_size len);
```

Semantics:

- attempts to read up to `len` bytes into `buf`
- returns the number of bytes read
- may return `0` for end-of-input or no data, depending on host policy
- may return a negative error code on failure

Intended use:

- basic `stdin`-style wrappers
- shell-like interactive programs

### 1.4 `ram_alloc`

Allocates a RAM block owned by the current FAE execution context.

Prototype:

```c
void *fae_core_ram_alloc(fae_size size, fae_size align);
```

Semantics:

- allocates at least `size` bytes
- the returned pointer must satisfy `align`
- returns `NULL` on failure
- alignment must be a non-zero power of two

Intended use:

- C `malloc`-style wrappers
- Rust `GlobalAlloc`
- runtime metadata allocations

### 1.5 `ram_free`

Frees a block previously allocated by `ram_alloc` or `ram_realloc`.

Prototype:

```c
void fae_core_ram_free(void *ptr, fae_size size, fae_size align);
```

Semantics:

- `ptr == NULL` is allowed and is a no-op
- `size` and `align` must match the allocation contract expected by the host allocator
- behavior is undefined if `ptr` was not allocated by this runtime family

### 1.6 `ram_realloc`

Resizes a previously allocated RAM block.

Prototype:

```c
void *fae_core_ram_realloc(
    void *ptr,
    fae_size old_size,
    fae_size new_size,
    fae_size align
);
```

Semantics:

- equivalent to `ram_alloc(new_size, align)` when `ptr == NULL`
- may move the allocation
- returns `NULL` on failure, leaving the original allocation valid
- `old_size` and `align` must match the previous allocation contract

Intended use:

- C `realloc` wrappers
- Rust container growth through runtime allocation layers

### 1.7 Core Family Rationale

### 1.7 `memory_layout`

Returns a host-defined table describing the memory zones visible to the current FAE execution context.

Purpose:

- allow runtimes to understand which memory regions exist
- identify executable, writable, shared, and device-backed areas
- support runtime policy decisions without hard-coding board-specific addresses

This is especially useful for:

- Rust and C allocators that want to reason about usable RAM
- runtime support for future shared-memory or IPC models
- debugging and diagnostics

Public data structures:

```c
enum fae_mem_type_e {
    FAE_MEM_CODE = 1,
    FAE_MEM_RAM = 2,
    FAE_MEM_NVM = 3,
    FAE_MEM_MMIO = 4,
    FAE_MEM_SHARED = 5,
    FAE_MEM_RESERVED = 6
};

enum fae_mem_rights_e {
    FAE_MEM_R = 1u << 0,
    FAE_MEM_W = 1u << 1,
    FAE_MEM_X = 1u << 2
};

enum fae_mem_sharing_e {
    FAE_MEM_PRIVATE = 0,
    FAE_MEM_SHARED_READONLY = 1,
    FAE_MEM_SHARED_MUTABLE = 2,
    FAE_MEM_HOST_ONLY = 3
};

struct fae_memory_zone {
    fae_addr base;
    fae_size size;
    fae_mem_type type;
    fae_mem_rights rights;
    fae_mem_sharing sharing;
};
```

Prototype:

```c
fae_i32 fae_core_memory_layout(
    struct fae_memory_zone *zones,
    fae_size capacity,
    fae_size *count
);
```

Semantics:

- if `zones == NULL`, the call writes the total required zone count to `*count`
- if `zones != NULL`, the call writes up to `capacity` entries into `zones`
- on success, `*count` receives the total number of zones known to the host
- the function returns `0` on success
- it returns an error if `count == NULL`
- it returns an error if `capacity` is too small for the chosen transfer mode

Recommended behavioral contract:

- zones are returned in ascending base-address order
- zones do not overlap
- `base + size` must not overflow the address type
- rights describe effective rights from the FAE program point of view
- sharing describes visibility and mutability policy, not just hardware attributes

Notes:

- the exact list of memory types may grow over time
- the initial set should remain stable enough for runtimes to branch on broad categories
- this call is descriptive only; it does not itself modify mappings or permissions

This set is intentionally small.

It is expected to be enough to support:

- panic output in Rust and C
- dynamic allocation in Rust (`alloc`) and C
- minimal interactive console use
- process termination
- memory-region-aware runtime initialization

It is not intended to provide:

- formatting
- environment variables
- clocks
- threads
- signals

Those can be layered above, or specified later in separate families.

## 2. Filesystem Syscalls

The `fs` family is the minimal filesystem layer intended to wrap XiPFS.

This family is specified as a public runtime ABI, not as a direct re-export of the internal XiPFS C API.

In particular:

- the mount object is not exposed in the syscall ABI
- the mount is selected by the host-side wrapper
- file and directory descriptors are exposed as opaque handles

The underlying host implementation may internally delegate to XiPFS calls such as:

- `xipfs_open`
- `xipfs_read`
- `xipfs_write`
- `xipfs_stat`
- `xipfs_statvfs`
- `xipfs_readdir`

but those internal details are not part of this ABI.

### 2.1 Exposed Opaque Types

The public ABI should expose only opaque descriptor handles.

Recommended abstract types:

```c
typedef fae_handle_t fae_file_t;
typedef fae_handle_t fae_dir_t;
```

These handles:

- are created by the runtime environment
- are consumed only by `fs` syscalls
- must not be dereferenced by user code
- are invalid after close

This deliberately hides internal XiPFS implementation types such as:

- `xipfs_file_desc_t`
- `xipfs_dir_desc_t`

### 2.2 Exposed Data Structures

Some filesystem calls need stable shared structures.

The ABI should expose dedicated public structures instead of raw internal XiPFS types where possible.

#### `fae_dirent`

Minimal directory entry:

```c
struct fae_dirent {
    char name[256];
};
```

Notes:

- the exact maximum name length should later be synchronized with the chosen path ABI
- for now, a fixed-size structure is preferred for ABI simplicity

#### `fae_statvfs`

Filesystem capacity information:

```c
struct fae_statvfs {
    fae_u32 f_bsize;
    fae_u32 f_frsize;
    fae_u32 f_blocks;
    fae_u32 f_bfree;
    fae_u32 f_bavail;
    fae_u32 f_flag;
    fae_u32 f_namemax;
};
```

Notes:

- this mirrors the currently relevant XiPFS information
- a future 64-bit-capable ABI may revise these field widths

#### `fae_stat`

This specification assumes a stable public `struct stat`-like type will be exposed.

For now:

- the exact layout is intentionally left open
- the runtime ABI should not expose the private internal XiPFS structures directly
- the chosen `fae_stat` layout must be documented explicitly when fixed

So `stat`-related calls are included in the function list below, but their structure layout remains `TODO`.

### 2.3 `fs_open`

Prototype:

```c
fae_i32 fae_fs_open(fae_file_t *out, const char *path, fae_flags flags, fae_mode mode);
```

Semantics:

- opens an existing file or creates one according to `flags`
- writes an opaque handle into `*out`
- returns `0` on success
- returns a negative or non-zero error code on failure

### 2.4 `fs_close`

Prototype:

```c
fae_i32 fae_fs_close(fae_file_t file);
```

Semantics:

- closes a previously opened file handle
- invalidates the handle

### 2.5 `fs_read`

Prototype:

```c
fae_ssize fae_fs_read(fae_file_t file, void *dst, fae_size nbytes);
```

Semantics:

- reads up to `nbytes` into `dst`
- returns the number of bytes read

### 2.6 `fs_write`

Prototype:

```c
fae_ssize fae_fs_write(fae_file_t file, const void *src, fae_size nbytes);
```

Semantics:

- writes up to `nbytes` from `src`
- returns the number of bytes written

### 2.7 `fs_lseek`

Prototype:

```c
fae_off fae_fs_lseek(fae_file_t file, fae_off off, fae_i32 whence);
```

Semantics:

- updates the current file position
- returns the resulting offset
- returns an invalid offset sentinel or error encoding on failure

### 2.8 `fs_fsync`

Prototype:

```c
fae_i32 fae_fs_fsync(fae_file_t file, fae_off pos);
```

Semantics:

- synchronizes the file state to storage
- `pos` keeps parity with the current XiPFS contract

### 2.9 `fs_fstat`

Prototype:

```c
fae_i32 fae_fs_fstat(fae_file_t file, struct fae_stat *buf);
```

Semantics:

- retrieves metadata for an open file

### 2.10 `fs_stat`

Prototype:

```c
fae_i32 fae_fs_stat(const char *path, struct fae_stat *buf);
```

### 2.11 `fs_statvfs`

Prototype:

```c
fae_i32 fae_fs_statvfs(const char *path, struct fae_statvfs *buf);
```

### 2.12 `fs_mkdir`

Prototype:

```c
fae_i32 fae_fs_mkdir(const char *path, fae_mode mode);
```

### 2.13 `fs_rmdir`

Prototype:

```c
fae_i32 fae_fs_rmdir(const char *path);
```

### 2.14 `fs_unlink`

Prototype:

```c
fae_i32 fae_fs_unlink(const char *path);
```

### 2.15 `fs_rename`

Prototype:

```c
fae_i32 fae_fs_rename(const char *from_path, const char *to_path);
```

### 2.16 `fs_new_file`

Prototype:

```c
fae_i32 fae_fs_new_file(const char *path, fae_u32 size, fae_u32 exec);
```

Semantics:

- creates a XiPFS file with reserved size and execution property
- this call is XiPFS-specific by design

### 2.17 `fs_opendir`

Prototype:

```c
fae_i32 fae_fs_opendir(fae_dir_t *out, const char *path);
```

### 2.18 `fs_readdir`

Prototype:

```c
fae_i32 fae_fs_readdir(fae_dir_t dir, struct fae_dirent *entry);
```

Semantics:

- reads the next directory entry into `entry`
- end-of-directory behavior must be documented by the implementation

### 2.19 `fs_closedir`

Prototype:

```c
fae_i32 fae_fs_closedir(fae_dir_t dir);
```

### 2.20 `fs_mount` and `fs_umount`

These calls need special treatment.

Internally, XiPFS uses:

- `xipfs_mount(xipfs_mount_t *mp)`
- `xipfs_umount(xipfs_mount_t *mp)`

But the public runtime ABI does not expose `xipfs_mount_t *`.

So the public contract should instead be:

```c
fae_i32 fae_fs_mount(void);
fae_i32 fae_fs_umount(void);
```

Semantics:

- operate on the host-selected default mount
- do not expose mount objects to applications

If multi-mount support is needed later, it should be introduced through a separate explicit mount-handle ABI rather than by leaking the internal XiPFS mount structure.

## 3. Network Family

The `network` family is intended to expose host-mediated communication primitives to FAE programs.

This family should not be designed as a full POSIX socket clone by default.

The preferred approach is:

- expose a small transport-oriented ABI first
- keep handles opaque
- let higher-level language runtimes build richer wrappers on top

Typical use cases:

- TCP/UDP-like communication
- local host channels
- RPC-style transports
- future service discovery or control channels

Recommended design constraints:

- opaque socket or endpoint handles
- explicit open / close / read / write style primitives
- no direct exposure of host-native socket structures
- clearly separated blocking / non-blocking policy

Status:

- `TODO`

Open questions:

- whether the first version should be socket-like or packet-like
- whether address structures should be generic or family-specific
- whether timeouts belong in `network` or in a future `time` family
- whether `poll`/`select`-like multiplexing is needed in the first version

## 4. Sensors Family

The `sensors` family is intended to expose host-managed sensor readings to FAE programs.

This family should be treated as a runtime service, not as direct MMIO.

The host remains responsible for:

- selecting which sensors are visible
- sampling or mediating access
- translating hardware-specific details into a stable ABI

Typical use cases:

- temperature readings
- IMU or motion sensors
- ADC-backed measurements
- battery or power state
- board- or platform-specific telemetry

Recommended design constraints:

- opaque sensor identifiers or host-defined numeric IDs
- explicit discovery before use
- structured metadata separate from sampled values
- avoid baking hardware register layouts into the ABI

Status:

- `TODO`

Open questions:

- whether sensors are addressed by numeric ID, string name, or both
- whether values are always scalar or may be vector samples
- whether units are normalized by the host or sensor-specific
- whether change notifications belong here or in a future event family

## 5. Families Not Yet Fully Specified

### 3.1 Threading

Thread support is intentionally left unspecified for now.

Status:

- `TODO`

This future family will likely need to answer at least:

- thread creation
- join/detach
- mutexes
- condition variables or wait queues
- thread-local or runtime-local state

It should be designed only after the `core` and `fs` families are exercised by actual C and Rust runtimes.

## 6. Intended Runtime Usage

### 4.1 C Runtime

Expected usage:

- `core` wraps into `exit`, console I/O, `malloc/free/realloc`
- `core` exposes memory topology information to the runtime if needed
- `fs` wraps into a libc-like file API
- the C runtime provides the bridge from FAE `start` to user `main`

### 4.2 Rust Runtime

Expected usage:

- `core` provides the allocator backend
- `core` provides panic/logging output
- `core` can expose memory zones to allocator or runtime policy code
- `fs` later backs a Rust filesystem layer
- the Rust runtime provides the bridge from FAE `start` to user `main`

This is the shortest path toward a Rust environment that is more ergonomic than raw `no_std`, without trying to reproduce the full hosted `std` model immediately.

## 7. Open Questions

The following points remain intentionally open:

- exact public `fae_stat` layout
- exact error-code model shared by `core` and `fs`
- exact path length limit and canonical path ABI
- end-of-directory convention for `fs_readdir`
- whether `fae_off` remains 32-bit permanently
- whether console I/O should be split into distinct `stdout` / `stderr` channels
- whether `memory_layout` should expose stable zone identifiers in addition to ordering
- the minimal first-version ABI shape for `network`
- the minimal first-version ABI shape for `sensors`
- future threading family

These should be resolved incrementally as the C and Rust runtime layers are implemented.
