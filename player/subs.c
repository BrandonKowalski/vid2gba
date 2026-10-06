#include "subs.h"

#define OAM ((volatile u32 *)0x07000000)
#define PAL_OBJ ((volatile u16 *)0x05000200)
#define OBJ_TILES ((volatile u32 *)0x06014000)
#define SUB_OAM 60
#define MAX_SPRITES 16
#define REGION_A 256
#define REGION_B 384
#define STRIDE 260

struct Cue {
    u32 start, end, data_off;
    u8 sprites, w, h, pad;
};

static const u8 *base;
static const struct Cue *cues;
static u32 count;
static u32 next;
static int shown = -1;
static int region;
static int bottom = 156;
static u32 shadow[MAX_SPRITES * 2];
static int nshadow;
static volatile int dirty;

void subs_init(const u8 *b, u32 off, u32 n)
{
    base = b;
    count = n;
    next = 0;
    shown = -1;
    nshadow = 0;
    if (!n) {
        for (int i = 0; i < MAX_SPRITES; i++) {
            OAM[(SUB_OAM + i) * 2] = 0x0200;
            OAM[(SUB_OAM + i) * 2 + 1] = 0;
        }
        return;
    }
    const u16 *pal = (const u16 *)(b + off);
    for (int i = 0; i < 16; i++)
        PAL_OBJ[16 + i] = pal[i];
    cues = (const struct Cue *)(b + off + 32);
    for (int i = 0; i < MAX_SPRITES; i++) {
        OAM[(SUB_OAM + i) * 2] = 0x0200;
        OAM[(SUB_OAM + i) * 2 + 1] = 0;
    }
}

void subs_seek(u32 clock)
{
    u32 lo = 0;
    u32 hi = count;
    while (lo < hi) {
        u32 mid = (lo + hi) >> 1;
        if (cues[mid].end <= clock)
            lo = mid + 1;
        else
            hi = mid;
    }
    next = lo;
}

void subs_hide(void)
{
    dirty = 0;
    nshadow = 0;
    shown = -1;
    dirty = 1;
}

void subs_update(u32 clock, int enabled, int overlay_up)
{
    if (!count)
        return;
    while (next < count && cues[next].end <= clock)
        next++;
    int want = (enabled && next < count && cues[next].start <= clock) ? (int)next : -1;
    int y_bottom = overlay_up ? 146 : 156;
    if (want == shown && y_bottom == bottom)
        return;
    if (want < 0) {
        bottom = y_bottom;
        if (shown >= 0)
            subs_hide();
        return;
    }
    dirty = 0;
    const struct Cue *c = &cues[want];
    const u8 *d = base + c->data_off;
    u32 tiles;
    if (want != shown) {
        tiles = region ? REGION_B : REGION_A;
        for (u32 s = 0; s < c->sprites; s++) {
            const u32 *src = (const u32 *)(d + s * STRIDE + 4);
            volatile u32 *dst = OBJ_TILES + (tiles + s * 8) * 8;
            for (int w = 0; w < 64; w++)
                dst[w] = src[w];
        }
        region ^= 1;
    } else {
        tiles = region ? REGION_A : REGION_B;
    }
    int x0 = 120 - (c->w >> 1);
    int y0 = y_bottom - c->h;
    for (u32 s = 0; s < c->sprites; s++) {
        u32 x = (u32)(x0 + d[s * STRIDE]) & 0x1FF;
        u32 y = (u32)(y0 + d[s * STRIDE + 1]) & 0xFF;
        shadow[s * 2] = y | (1u << 14) | (x << 16) | (2u << 30);
        shadow[s * 2 + 1] = (512 + tiles + s * 8) | (1u << 12);
    }
    nshadow = c->sprites;
    shown = want;
    bottom = y_bottom;
    dirty = 1;
}

void subs_vblank(void)
{
    if (!dirty)
        return;
    for (int i = 0; i < MAX_SPRITES; i++) {
        OAM[(SUB_OAM + i) * 2] = i < nshadow ? shadow[i * 2] : 0x0200;
        OAM[(SUB_OAM + i) * 2 + 1] = shadow[i * 2 + 1];
    }
    dirty = 0;
}
