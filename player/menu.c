#include "menu.h"

#define OAM ((volatile u32 *)0x07000000)
#define PAL_OBJ ((volatile u16 *)0x05000200)
#define OBJ_TILES ((volatile u32 *)0x06014000)
#define MENU_TILE_BASE 32
#define MENU_OAM 30
#define STRIP 5
#define ROWS 5

static u32 shadow[ROWS * STRIP * 2];
static volatile int dirty;
static volatile int visible;

void menu_init(const u8 *data, u32 tiles)
{
    const u16 *pal = (const u16 *)data;
    for (int i = 0; i < 32; i++)
        PAL_OBJ[32 + i] = pal[i];
    const u32 *src = (const u32 *)(data + 64);
    for (u32 i = 0; i < tiles * 8; i++)
        OBJ_TILES[MENU_TILE_BASE * 8 + i] = src[i];
    for (int i = 0; i < ROWS * STRIP; i++) {
        OAM[(MENU_OAM + i) * 2] = 0x0200;
        OAM[(MENU_OAM + i) * 2 + 1] = 0;
    }
}

void menu_show(const u8 *labels, int count, int selected)
{
    dirty = 0;
    int y0 = 80 - count * 6;
    for (int r = 0; r < ROWS; r++) {
        for (int s = 0; s < STRIP; s++) {
            int i = (r * STRIP + s) * 2;
            if (r < count) {
                u32 y = (u32)(y0 + r * 12) & 0xFF;
                u32 x = (u32)(40 + s * 32);
                shadow[i] = y | (1u << 14) | (x << 16) | (1u << 30);
                shadow[i + 1] = (512 + MENU_TILE_BASE + labels[r] * 20 + s * 4) | ((r == selected ? 3u : 2u) << 12);
            } else {
                shadow[i] = 0x0200;
                shadow[i + 1] = 0;
            }
        }
    }
    visible = 1;
    dirty = 1;
}

void menu_hide(void)
{
    visible = 0;
    dirty = 1;
}

void menu_vblank(void)
{
    if (!dirty)
        return;
    for (int i = 0; i < ROWS * STRIP; i++) {
        OAM[(MENU_OAM + i) * 2] = visible ? shadow[i * 2] : 0x0200;
        OAM[(MENU_OAM + i) * 2 + 1] = shadow[i * 2 + 1];
    }
    dirty = 0;
}
