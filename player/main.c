#include "gba.h"
#include "video.h"
#include "audio.h"
#include "overlay.h"
#include "menu.h"
#include "subs.h"

#define SPLASH_FPS 20
#define SPLASH_SPF 43840307u
#define OVERLAY_FRAMES 120
#define REPEAT_FRAMES 18

struct Container {
    char magic[8];
    u32 version, flags, fps_num, fps_den, frame_count;
    u16 width, height, x_off, y_off;
    u32 audio_rate, audio_len, audio_off, frames_off, spf_q16, title_off;
    char title[32];
    u32 splash_frame_count, splash_frames_off, splash_audio_off, splash_audio_len;
    u32 overlay_off, overlay_tile_count, total_samples;
    u32 subtitle_count, subtitle_off;
    u16 fill_pa, fill_pad;
    s32 fill_x, fill_y;
    u32 menu_off, menu_tile_count;
    u32 video_count, videos_off, menu_pages_off;
};

struct VideoEntry {
    u32 frame_count, frames_off, audio_off, audio_len, spf_q16, total_samples;
    u16 width, height, x_off, y_off;
    u32 flags;
    u16 fill_pa, fill_pad;
    s32 fill_x, fill_y;
    u32 subtitle_count, subtitle_off, fps, reserved;
};

struct Stream {
    const u32 *table;
    u32 count;
    const u8 *audio;
    u32 audio_len;
    u32 spf;
    u32 fps;
    u32 total;
    struct Layout layout;
    int half;
    int zoomable;
    u16 fill_pa;
    s32 fill_x, fill_y;
    u32 subtitle_count, subtitle_off;
};

struct Slot {
    char tag[8];
    u32 offset;
};

__attribute__((used, aligned(4))) const volatile struct Slot container_slot = {"VGBAPTR", 0};

volatile u32 vblanks;
static const struct Container *box;
static u16 keys_prev;
static u16 keys_held;

void *memcpy(void *d, const void *s, unsigned int n)
{
    u8 *a = d;
    const u8 *b = s;
    while (n--)
        *a++ = *b++;
    return d;
}

void *memset(void *d, int c, unsigned int n)
{
    u8 *a = d;
    while (n--)
        *a++ = (u8)c;
    return d;
}

void isr(u32 flags)
{
    if (flags & IRQ_VBLANK) {
        vblanks++;
        audio_vblank();
        video_vblank();
        overlay_vblank();
        menu_vblank();
        subs_vblank();
    }
}

static u16 keys_pressed(void)
{
    u16 k = ~REG_KEYINPUT & 0x03FF;
    u16 p = k & ~keys_prev;
    keys_prev = k;
    keys_held = k;
    return p;
}

static void reset_keys(void)
{
    keys_prev = ~REG_KEYINPUT & 0x03FF;
    keys_held = keys_prev;
}

static void clear_pages(void)
{
    volatile u32 *v = (volatile u32 *)0x06000000;
    for (u32 i = 0; i < 0x5000; i++)
        v[i] = 0;
}

static void set_scale(int half)
{
    u16 s = half ? 0x80 : 0x100;
    REG_BG2PA = s;
    REG_BG2PB = 0;
    REG_BG2PC = 0;
    REG_BG2PD = s;
    REG_BG2X = 0;
    REG_BG2Y = 0;
}

static int valid(const struct Container *c)
{
    static const char magic[8] = "VID2GBA";
    for (int i = 0; i < 8; i++)
        if (c->magic[i] != magic[i])
            return 0;
    return c->version == 4;
}

static void show_error(void)
{
    REG_DISPCNT = 0x0404;
    set_scale(0);
    clear_pages();
    PAL_BG[0] = 0x001F;
    for (;;)
        vblank_wait();
}

static void show_title(void)
{
    const u8 *t = (const u8 *)box + box->title_off;
    const u16 *pal = (const u16 *)t;
    const u32 *img = (const u32 *)(t + 512);
    volatile u32 *dst = (volatile u32 *)0x06000000;
    REG_DISPCNT = 0x0404;
    set_scale(0);
    for (int i = 0; i < 256; i++)
        PAL_BG[i] = pal[i];
    for (int i = 0; i < 240 * 160 / 4; i++)
        dst[i] = img[i];
    reset_keys();
    for (;;) {
        vblank_wait();
        if (keys_pressed() & KEY_START)
            return;
    }
}

static int subs_on = 1;
static int fill_mode = 0;

static void set_picture(const struct Stream *s, int fill)
{
    if (!fill || !s->zoomable) {
        set_scale(s->half);
        return;
    }
    REG_BG2PA = s->fill_pa;
    REG_BG2PB = 0;
    REG_BG2PC = 0;
    REG_BG2PD = s->fill_pa;
    REG_BG2X = (u32)s->fill_x;
    REG_BG2Y = (u32)s->fill_y;
}

static int run_menu(const struct Stream *s)
{
    int sel = 0;
    REG_BLDCNT = 0x00C4;
    REG_BLDY = 10;
    for (;;) {
        u8 items[5];
        u8 kinds[5];
        int n = 0;
        items[n] = 0;
        kinds[n++] = 0;
        if (s->subtitle_count) {
            items[n] = subs_on ? 1 : 2;
            kinds[n++] = 1;
        }
        items[n] = fill_mode ? 4 : 3;
        kinds[n++] = 2;
        if (box->video_count > 1) {
            items[n] = 6;
            kinds[n++] = 4;
        }
        items[n] = 5;
        kinds[n++] = 3;
        menu_show(items, n, sel);
        vblank_wait();
        u16 k = keys_pressed();
        int result = -1;
        if (k & (KEY_SELECT | KEY_B))
            result = 0;
        if (k & KEY_UP)
            sel = sel ? sel - 1 : n - 1;
        if (k & KEY_DOWN)
            sel = sel + 1 == n ? 0 : sel + 1;
        if (k & KEY_A) {
            if (kinds[sel] == 0)
                result = 0;
            else if (kinds[sel] == 1)
                subs_on = !subs_on;
            else if (kinds[sel] == 2) {
                fill_mode = !fill_mode;
                set_picture(s, fill_mode);
            } else if (kinds[sel] == 4)
                result = 2;
            else
                result = 1;
        }
        if (result >= 0) {
            menu_hide();
            REG_BLDCNT = 0;
            return result;
        }
    }
}

static u64 mul_u32(u32 a, u32 b)
{
    u64 r = 0;
    u64 x = a;
    while (b) {
        if (b & 1)
            r += x;
        x <<= 1;
        b >>= 1;
    }
    return r;
}

static const u8 *record(const struct Stream *s, u32 n)
{
    return (const u8 *)box + s->table[n];
}

static u32 gop_before(const struct Stream *s, u32 n)
{
    while (n > 0 && !(record(s, n)[0] & 2))
        n--;
    return n;
}

static u32 gop_after(const struct Stream *s, u32 n)
{
    while (n < s->count && !(record(s, n)[0] & 2))
        n++;
    return n;
}

static int play(const struct Stream *s, int controls)
{
    REG_DISPCNT = 0x1454;
    set_picture(s, fill_mode);
    video_reset();
    clear_pages();
    PAL_BG[0] = 0;
    audio_start(s->audio, s->audio_len);
    subs_init((const u8 *)box, s->subtitle_off, s->subtitle_count);
    subs_seek(0);
    reset_keys();
    u32 n = 0;
    u64 due = 0;
    int paused = 0;
    int show_one = 0;
    u32 repeat_at = 0;
    u32 drawn = vblanks - 1;
    while (n < s->count) {
        u16 k = keys_pressed();
        if (!controls) {
            if (k & (KEY_START | KEY_A)) {
                audio_stop();
                return 2;
            }
        } else {
            if (k & KEY_SELECT) {
                if (!paused)
                    audio_pause();
                overlay_hide();
                subs_hide();
                int r = run_menu(s);
                set_picture(s, fill_mode);
                reset_keys();
                if (r == 1 || r == 2) {
                    audio_stop();
                    overlay_hide();
                    subs_hide();
                    return r == 1 ? 1 : 3;
                }
                if (!paused)
                    audio_resume();
                drawn = vblanks - 1;
            }
            if (k & (KEY_L | KEY_R)) {
                fill_mode = (k & KEY_L) ? 1 : 0;
                set_picture(s, fill_mode);
                overlay_show_for(OVERLAY_FRAMES);
                drawn = vblanks - 1;
            }
            if (k & (KEY_START | KEY_A)) {
                paused = !paused;
                if (paused)
                    audio_pause();
                else
                    audio_resume();
                overlay_show_for(OVERLAY_FRAMES);
                drawn = vblanks - 1;
            }
            int dir = 0;
            if (k & (KEY_LEFT | KEY_RIGHT)) {
                dir = (k & KEY_LEFT) ? -1 : 1;
                repeat_at = vblanks + REPEAT_FRAMES;
            } else if ((keys_held & (KEY_LEFT | KEY_RIGHT)) && (s32)(vblanks - repeat_at) >= 0) {
                dir = (keys_held & KEY_LEFT) ? -1 : 1;
                repeat_at = vblanks + REPEAT_FRAMES;
            }
            if (dir) {
                u32 cur = n ? n - 1 : 0;
                u32 step = 5 * s->fps;
                u32 g;
                if (dir < 0)
                    g = gop_before(s, cur > step ? cur - step : 0);
                else
                    g = cur + step >= s->count ? s->count : gop_after(s, cur + step);
                if (g >= s->count)
                    break;
                while (video_flip_pending())
                    vblank_wait();
                due = mul_u32(g, s->spf);
                audio_seek(s->audio, s->audio_len, (u32)(due >> 16), paused);
                subs_seek((u32)(due >> 16));
                clear_pages();
                n = g;
                show_one = 1;
                overlay_show_for(OVERLAY_FRAMES);
                drawn = vblanks - 1;
            }
            if (overlay_visible() && drawn != vblanks) {
                drawn = vblanks;
                overlay_update(audio_clock(), s->total, box->audio_rate, paused);
            }
            subs_update(audio_clock(), subs_on, overlay_visible());
        }
        int ready = show_one || (!paused && ((u64)audio_clock() << 16) >= due);
        if (ready && !video_flip_pending()) {
            const u16 *pal;
            video_decode(record(s, n), VRAM + (n & 1) * PAGE_U16, &s->layout, &pal);
            video_request_flip(n & 1, pal);
            n++;
            due += s->spf;
            show_one = 0;
        } else {
            vblank_wait();
        }
    }
    while (!paused && ((u64)audio_clock() << 16) < due)
        vblank_wait();
    while (video_flip_pending())
        vblank_wait();
    audio_stop();
    overlay_hide();
    subs_hide();
    return 0;
}

static void video_stream(struct Stream *s, u32 i)
{
    const struct VideoEntry *v = (const struct VideoEntry *)((const u8 *)box + box->videos_off) + i;
    s->table = (const u32 *)((const u8 *)box + v->frames_off);
    s->count = v->frame_count;
    s->audio = (const u8 *)box + v->audio_off;
    s->audio_len = v->audio_len;
    s->spf = v->spf_q16;
    s->fps = v->fps;
    s->total = v->total_samples;
    s->layout.w = v->width;
    s->layout.h = v->height;
    s->layout.x_off = v->x_off;
    s->layout.y_off = v->y_off;
    s->half = v->flags & 1;
    s->zoomable = 1;
    s->fill_pa = v->fill_pa;
    s->fill_x = v->fill_x;
    s->fill_y = v->fill_y;
    s->subtitle_count = v->subtitle_count;
    s->subtitle_off = v->subtitle_off;
}

#define MENU_PAGE_BYTES (512 + 240 * 160)

static const u8 *menu_page(u32 page)
{
    return (const u8 *)box + box->menu_pages_off + 4 + page * MENU_PAGE_BYTES;
}

static void show_menu_page(u32 page)
{
    const u8 *p = menu_page(page);
    const u16 *pal = (const u16 *)p;
    const u32 *img = (const u32 *)(p + 512);
    volatile u32 *dst = (volatile u32 *)0x06000000;
    vblank_wait();
    for (int i = 0; i < 256; i++)
        PAL_BG[i] = 0;
    REG_BLDCNT = 0;
    REG_DISPCNT = 0x0404;
    set_scale(0);
    for (int i = 0; i < 240 * 160 / 4; i++)
        dst[i] = img[i];
    vblank_wait();
    for (int i = 0; i < 256; i++)
        PAL_BG[i] = pal[i];
}

static u32 main_menu(u32 sel)
{
    u32 n = box->video_count;
    u32 page = 0xFFFFFFFFu;
    reset_keys();
    for (;;) {
        u32 p = 0;
        u32 row = sel;
        while (row >= 3) {
            row -= 3;
            p++;
        }
        if (p != page) {
            page = p;
            show_menu_page(page);
        } else {
            vblank_wait();
        }
        const u16 *pal = (const u16 *)menu_page(page);
        for (u32 r = 0; r < 3; r++)
            PAL_BG[5 + r] = r == row ? pal[8] : pal[5];
        u16 k = keys_pressed();
        if (k & (KEY_A | KEY_START))
            return sel;
        if (k & KEY_DOWN)
            sel = sel + 1 >= n ? 0 : sel + 1;
        if (k & KEY_UP)
            sel = sel ? sel - 1 : n - 1;
    }
}

static void splash_stream(struct Stream *s)
{
    s->table = (const u32 *)((const u8 *)box + box->splash_frames_off);
    s->count = box->splash_frame_count;
    s->audio = (const u8 *)box + box->splash_audio_off;
    s->audio_len = box->splash_audio_len;
    s->spf = SPLASH_SPF;
    s->fps = SPLASH_FPS;
    s->total = box->splash_audio_len;
    s->layout.w = 240;
    s->layout.h = 160;
    s->layout.x_off = 0;
    s->layout.y_off = 0;
    s->half = 0;
    s->zoomable = 0;
    s->fill_pa = 256;
    s->fill_x = 0;
    s->fill_y = 0;
    s->subtitle_count = 0;
    s->subtitle_off = 0;
}

int main(void)
{
    REG_WAITCNT = 0x4317;
    REG_DISPSTAT = 0x0008;
    REG_IE = IRQ_VBLANK;
    REG_IME = 1;
    u32 off = container_slot.offset;
    box = (const struct Container *)off;
    if (off < 0x08000000 || !valid(box))
        show_error();
    overlay_init((const u8 *)box + box->overlay_off, box->overlay_tile_count);
    menu_init((const u8 *)box + box->menu_off, box->menu_tile_count);
    subs_init((const u8 *)box, 0, 0);
    struct Stream splash;
    struct Stream movie;
    splash_stream(&splash);
    play(&splash, 0);
    if (box->video_count <= 1) {
        video_stream(&movie, 0);
        for (;;) {
            show_title();
            while (play(&movie, 1) == 1)
                ;
        }
    }
    u32 sel = 0;
    for (;;) {
        sel = main_menu(sel);
        video_stream(&movie, sel);
        int r;
        do {
            r = play(&movie, 1);
        } while (r == 1);
        if (r == 0)
            sel = sel + 1 >= box->video_count ? 0 : sel + 1;
    }
}
