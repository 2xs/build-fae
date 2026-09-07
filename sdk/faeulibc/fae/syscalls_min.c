#include <stdint.h>

int write_err_hs(int c);

static void write_status_unsigned(unsigned value)
{
    char digits[10];
    unsigned i = 0;

    do {
        digits[i++] = (char)('0' + (value % 10u));
        value /= 10u;
    } while (value != 0u);

    while (i != 0u) {
        write_err_hs(digits[--i]);
    }
}

_Noreturn void _exit(int status)
{
    unsigned code = (unsigned)status;

    write_err_hs('[');
    write_err_hs('e');
    write_err_hs('x');
    write_err_hs('i');
    write_err_hs('t');
    write_err_hs('=');
    write_status_unsigned(code);
    write_err_hs(']');
    write_err_hs('\n');

    for (;;) {
    }
}
