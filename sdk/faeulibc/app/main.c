#include <stdio.h>

int main(int argc, char **argv)
{
    (void)argc;
    (void)argv;

    puts("faeulibc: stdout via picolibc");
    fputs("faeulibc: stderr via picolibc\n", stderr);

    return 0;
}
