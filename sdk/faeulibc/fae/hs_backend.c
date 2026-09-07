#include <stdint.h>

enum {
    SYS_WRITEC = 0x03,
    SYS_READC = 0x07,
};

static inline int semihost_call(int op, void *arg)
{
    register int r0 __asm("r0") = op;
    register void *r1 __asm("r1") = arg;

    __asm__ volatile("bkpt 0xab" : "+r"(r0) : "r"(r1) : "memory");
    return r0;
}

int write_hs(int c)
{
    unsigned char ch = (unsigned char)c;

    semihost_call(SYS_WRITEC, &ch);
    return ch;
}

int write_err_hs(int c)
{
    /*
     * Semihosting only exposes one console stream in this starter, so stderr
     * currently shares the same backend as stdout.
     */
    return write_hs(c);
}

int read_hs(void)
{
    return semihost_call(SYS_READC, 0);
}
