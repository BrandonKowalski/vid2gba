#ifndef AUDIO_H
#define AUDIO_H

#include "gba.h"

void audio_start(const u8 *data, u32 samples);
void audio_pause(void);
void audio_resume(void);
void audio_stop(void);
u32 audio_clock(void);
void audio_vblank(void);
void audio_seek(const u8 *data, u32 total, u32 sample, int paused);

#endif
