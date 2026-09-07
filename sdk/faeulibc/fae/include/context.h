#ifndef FAEULIBC_CONTEXT_H
#define FAEULIBC_CONTEXT_H

#include <stdint.h>

typedef struct context_s {
    void *file_base;
    uint8_t is_safe_call;
    void *syscall_table;
    int32_t argc;
    char *argv[64];
    const void *former_got;
    const void *current_got;
} context_s;

#endif
