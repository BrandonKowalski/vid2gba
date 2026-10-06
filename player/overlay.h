#ifndef OVERLAY_H
#define OVERLAY_H

#include "gba.h"

void overlay_init(const u8 *data, u32 tiles);
void overlay_update(u32 elapsed, u32 total, u32 rate, int paused);
void overlay_show_for(u32 frames);
void overlay_hide(void);
int overlay_visible(void);
void overlay_vblank(void);

#endif
