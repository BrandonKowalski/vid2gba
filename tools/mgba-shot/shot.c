#include <mgba/core/core.h>
#include <mgba/core/config.h>
#include <mgba/core/log.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>

struct Press {
    int bit;
    int from;
    int to;
};

static void quiet(struct mLogger *logger, int category, enum mLogLevel level, const char *format, va_list args)
{
    (void)logger;
    (void)category;
    (void)level;
    (void)format;
    (void)args;
}

static struct mLogger logger = {.log = quiet};

static int key_bit(const char *name)
{
    static const char *names[] = {"a", "b", "select", "start", "right", "left", "up", "down", "r", "l"};
    for (int i = 0; i < 10; i++)
        if (!strcmp(name, names[i]))
            return i;
    return -1;
}

static int write_shot(const char *dir, int index, const color_t *buf)
{
    char path[4096];
    snprintf(path, sizeof(path), "%s/shot%d.rgb", dir, index);
    FILE *f = fopen(path, "wb");
    if (!f)
        return 0;
    for (int i = 0; i < 240 * 160; i++) {
        unsigned char px[3] = {buf[i] & 0xFF, (buf[i] >> 8) & 0xFF, (buf[i] >> 16) & 0xFF};
        fwrite(px, 1, 3, f);
    }
    fclose(f);
    return 1;
}

int main(int argc, char **argv)
{
    mLogSetDefaultLogger(&logger);
    if (argc < 4) {
        fprintf(stderr, "usage: %s ROM OUTDIR SPEC...\n", argv[0]);
        return 2;
    }
    struct Press presses[64];
    int shots[64];
    int np = 0;
    int ns = 0;
    int last = 0;
    for (int i = 3; i < argc && np < 64 && ns < 64; i++) {
        char *at = strchr(argv[i], '@');
        if (at) {
            *at = 0;
            int bit = key_bit(argv[i]);
            if (bit < 0) {
                fprintf(stderr, "unknown key %s\n", argv[i]);
                return 2;
            }
            char *plus = strchr(at + 1, '+');
            int from = atoi(at + 1);
            int to = from + (plus ? atoi(plus + 1) : 6);
            presses[np++] = (struct Press){bit, from, to};
            if (to > last)
                last = to;
        } else {
            shots[ns] = atoi(argv[i]);
            if (shots[ns] > last)
                last = shots[ns];
            ns++;
        }
    }
    struct mCore *core = mCoreFind(argv[1]);
    if (!core || !core->init(core)) {
        fprintf(stderr, "cannot create core for %s\n", argv[1]);
        return 1;
    }
    mCoreInitConfig(core, NULL);
    color_t *buf = calloc(240 * 160, sizeof(color_t));
    core->setVideoBuffer(core, buf, 240);
    if (!mCoreLoadFile(core, argv[1])) {
        fprintf(stderr, "cannot load %s\n", argv[1]);
        return 1;
    }
    core->reset(core);
    for (int frame = 1; frame <= last; frame++) {
        uint32_t keys = 0;
        for (int p = 0; p < np; p++)
            if (frame >= presses[p].from && frame < presses[p].to)
                keys |= 1u << presses[p].bit;
        core->setKeys(core, keys);
        core->runFrame(core);
        for (int s = 0; s < ns; s++)
            if (shots[s] == frame && !write_shot(argv[2], s + 1, buf))
                return 1;
    }
    core->deinit(core);
    free(buf);
    return 0;
}
