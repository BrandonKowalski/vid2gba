#include "overlay.h"

#define OAM ((volatile u32 *)0x07000000)
#define PAL_OBJ ((volatile u16 *)0x05000200)
#define OBJ_TILES ((volatile u32 *)0x06014000)
#define SPRITES 30
#define ROW_Y 150
#define ROW_X 8
#define BAR_TILES 13
#define BAR_PX (BAR_TILES * 8)
#define T_COLON 10
#define T_SLASH 11
#define T_BLANK 12
#define T_BAR 13
#define T_PAUSE 22

extern volatile u32 vblanks;

static u32 shadow[SPRITES * 2];
static volatile int dirty;
static volatile int visible;
static volatile int sticky;
static u32 hide_at;

static u32 bios_div(u32 n, u32 d, u32 *rem)
{
    register u32 r0 __asm__("r0") = n;
    register u32 r1 __asm__("r1") = d;
    __asm__ volatile("swi 0x06" : "+r"(r0), "+r"(r1) : : "r2", "r3");
    if (rem)
        *rem = r1;
    return r0;
}

void overlay_init(const u8 *data, u32 tiles)
{
    const u16 *pal = (const u16 *)data;
    for (int i = 0; i < 16; i++)
        PAL_OBJ[i] = pal[i];
    const u32 *src = (const u32 *)(data + 32);
    for (u32 i = 0; i < tiles * 8; i++)
        OBJ_TILES[i] = src[i];
    for (int i = 0; i < 128; i++) {
        OAM[i * 2] = 0x0200;
        OAM[i * 2 + 1] = 0;
    }
}

static void put(int i, u32 x, u32 tile)
{
    shadow[i * 2] = ROW_Y | ((x & 0x1FF) << 16);
    shadow[i * 2 + 1] = 512 + tile;
}

static int put_time(int i, u32 x, u32 secs)
{
    u32 s;
    u32 m = bios_div(secs, 60, &s);
    if (m > 99)
        m = 99;
    u32 m1;
    u32 s1;
    u32 m10 = bios_div(m, 10, &m1);
    u32 s10 = bios_div(s, 10, &s1);
    put(i++, x, m10);
    put(i++, x + 8, m1);
    put(i++, x + 16, T_COLON);
    put(i++, x + 24, s10);
    put(i++, x + 32, s1);
    return i;
}

void overlay_update(u32 elapsed, u32 total, u32 rate, int paused)
{
    dirty = 0;
    if (elapsed > total)
        elapsed = total;
    int i = 0;
    u32 x = ROW_X;
    put(i++, x, paused ? T_PAUSE : T_BLANK);
    put(i++, x + 8, T_BLANK);
    i = put_time(i, x + 16, bios_div(elapsed, rate, 0));
    put(i++, x + 56, T_SLASH);
    i = put_time(i, x + 64, bios_div(total, rate, 0));
    put(i++, x + 104, T_BLANK);
    u32 denom = total >> 4;
    u32 fill = denom ? bios_div((elapsed >> 4) * BAR_PX, denom, 0) : 0;
    if (fill > BAR_PX)
        fill = BAR_PX;
    for (u32 t = 0; t < BAR_TILES; t++) {
        u32 k = fill > t * 8 ? fill - t * 8 : 0;
        if (k > 8)
            k = 8;
        put(i++, x + 112 + t * 8, T_BAR + k);
    }
    put(i++, x + 112 + BAR_TILES * 8, T_BLANK);
    for (; i < SPRITES; i++) {
        shadow[i * 2] = 0x0200;
        shadow[i * 2 + 1] = 0;
    }
    sticky = paused;
    dirty = 1;
}

void overlay_show_for(u32 frames)
{
    hide_at = vblanks + frames;
    visible = 1;
}

void overlay_hide(void)
{
    visible = 0;
    sticky = 0;
    dirty = 1;
}

int overlay_visible(void)
{
    return visible;
}

void overlay_vblank(void)
{
    if (visible && !sticky && (s32)(vblanks - hide_at) >= 0) {
        visible = 0;
        dirty = 1;
    }
    if (!dirty)
        return;
    for (int i = 0; i < SPRITES; i++) {
        OAM[i * 2] = visible ? shadow[i * 2] : 0x0200;
        OAM[i * 2 + 1] = shadow[i * 2 + 1];
    }
    dirty = 0;
}
