#include "video.h"

static volatile int pending;
static int pending_page;
static const u16 *pending_pal;

IWRAM_CODE void video_decode(const u8 *rec, volatile u16 *page, const struct Layout *l, const u16 **pal)
{
    u32 cb_len = rec[2] | (rec[3] << 8);
    const u8 *p = rec + 4;
    *pal = 0;
    if (rec[0] & 2) {
        *pal = (const u16 *)p;
        p += 512;
    }
    const u32 *cb = (const u32 *)p;
    p += cb_len * 4;
    u32 bw = l->w >> 1;
    u32 total = bw * (l->h >> 1);
    volatile u16 *row = page + ((l->y_off * 240 + l->x_off) >> 1);
    u32 bx = 0;
    u32 b = 0;
    while (b < total) {
        u32 op = *p++;
        if (op < 0x80) {
            u32 n = op + 1;
            b += n;
            bx += n;
            while (bx >= bw) {
                bx -= bw;
                row += 240;
            }
        } else {
            u32 n = op - 0x7F;
            b += n;
            while (n--) {
                u32 e = cb[*p++];
                row[bx] = e;
                row[bx + 120] = e >> 16;
                if (++bx == bw) {
                    bx = 0;
                    row += 240;
                }
            }
        }
    }
}

void video_request_flip(int page, const u16 *pal)
{
    pending_page = page;
    pending_pal = pal;
    pending = 1;
}

int video_flip_pending(void)
{
    return pending;
}

void video_vblank(void)
{
    if (!pending)
        return;
    if (pending_pal)
        for (int i = 0; i < 256; i++)
            PAL_BG[i] = pending_pal[i];
    REG_DISPCNT = (REG_DISPCNT & ~0x0010) | (pending_page ? 0x0010 : 0);
    pending = 0;
}

void video_reset(void)
{
    pending = 0;
}
