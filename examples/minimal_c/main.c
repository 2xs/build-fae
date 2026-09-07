volatile int COUNTER = 7;
volatile int SCRATCH;

int start(void)
{
    SCRATCH = COUNTER + 1;
    return SCRATCH;
}
