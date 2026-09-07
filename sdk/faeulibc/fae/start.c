#include "fae/include/context.h"

int main(int argc, char **argv);

int start(context_s *ctx)
{
    int argc = 0;
    char **argv = 0;

    if (ctx) {
        argc = ctx->argc;
        argv = ctx->argv;
    }

    return main(argc, argv);
}
