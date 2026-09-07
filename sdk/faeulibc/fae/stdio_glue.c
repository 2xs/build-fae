#include <stdio.h>

int read_hs(void);
int write_hs(int c);
int write_err_hs(int c);

static int fae_stdout_putc(char c, FILE *file)
{
    (void)file;
    return write_hs((unsigned char)c);
}

static int fae_stderr_putc(char c, FILE *file)
{
    (void)file;
    return write_err_hs((unsigned char)c);
}

static int fae_stdin_getc(FILE *file)
{
    (void)file;
    return read_hs();
}

static FILE fae_stdout_file = FDEV_SETUP_STREAM(fae_stdout_putc, 0, 0, _FDEV_SETUP_WRITE);
static FILE fae_stderr_file = FDEV_SETUP_STREAM(fae_stderr_putc, 0, 0, _FDEV_SETUP_WRITE);
static FILE fae_stdin_file = FDEV_SETUP_STREAM(0, fae_stdin_getc, 0, _FDEV_SETUP_READ);

FILE *const stdout = &fae_stdout_file;
FILE *const stderr = &fae_stderr_file;
FILE *const stdin = &fae_stdin_file;
