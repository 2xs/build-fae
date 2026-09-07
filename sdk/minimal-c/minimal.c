#include <stdint.h>

/*
 * Minimal runtime-facing context placeholder.
 *
 * The runtime ABI document currently specifies syscall families and common
 * conventions but does not yet freeze a C struct definition. This starter uses
 * the context shape already carried by the repository runtimes so application
 * code can accept a context pointer without depending on stdriot.
 */
typedef struct context_s {
    void *file_base;
    uint8_t is_safe_call;
    void *syscall_table;
    int32_t argc;
    char *argv[64];
    const void *former_got;
    const void *current_got;
} context_s;

static void sh_puts(const char *s)
{
    register uint32_t op __asm("r0") = 0x04;
    register const char *msg __asm("r1") = s;
    __asm__ volatile("bkpt 0xab" : : "r"(op), "r"(msg) : "memory");
}

int start(context_s *ctx)
{
    (void)ctx;
    sh_puts("Hello World!\n");
    return 0;
}
