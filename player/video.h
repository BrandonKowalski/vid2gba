#ifndef VIDEO_H
#define VIDEO_H

#include "gba.h"

struct Layout {
    u16 w, h, x_off, y_off;
};

IWRAM_CODE void video_decode(const u8 *rec, volatile u16 *page, const struct Layout *l, const u16 **pal);
void video_request_flip(int page, const u16 *pal);
int video_flip_pending(void);
void video_vblank(void);
void video_reset(void);

#endif
