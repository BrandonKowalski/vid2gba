#ifndef SUBS_H
#define SUBS_H

#include "gba.h"

void subs_init(const u8 *base, u32 off, u32 count);
void subs_seek(u32 clock);
void subs_update(u32 clock, int enabled, int overlay_up);
void subs_hide(void);
void subs_vblank(void);

#endif
