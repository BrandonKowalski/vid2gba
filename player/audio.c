#include "audio.h"

#define FRAME_SAMPLES 224

static const u16 steps[89] = {
    7, 8, 9, 10, 11, 12, 13, 14, 16, 17, 19, 21, 23, 25, 28, 31, 34, 37, 41, 45, 50, 55, 60, 66, 73, 80, 88, 97,
    107, 118, 130, 143, 157, 173, 190, 209, 230, 253, 279, 307, 337, 371, 408, 449, 494, 544, 598, 658, 724, 796,
    876, 963, 1060, 1166, 1282, 1411, 1552, 1707, 1878, 2066, 2272, 2499, 2749, 3024, 3327, 3660, 4026, 4428, 4871,
    5358, 5894, 6484, 7132, 7845, 8630, 9493, 10442, 11487, 12635, 13899, 15289, 16818, 18500, 20350, 22385, 24623,
    27086, 29794, 32767,
};
static const s8 index_table[16] = {-1, -1, -1, -1, 2, 4, 6, 8, -1, -1, -1, -1, 2, 4, 6, 8};

static s8 buffers[2][FRAME_SAMPLES] __attribute__((aligned(4)));
static int cur;
static volatile int playing;
static volatile u32 clock;
static const u8 *src;
static u32 remaining;
static u32 block_pos;
static s32 predictor;
static s32 step_index;

static void fill(s8 *out)
{
    for (int i = 0; i < FRAME_SAMPLES; i++) {
        if (!remaining) {
            out[i] = 0;
            continue;
        }
        if (block_pos == 0) {
            predictor = (s16)(src[0] | (src[1] << 8));
            step_index = src[2];
            src += 4;
        }
        u32 byte = src[block_pos >> 1];
        u32 nib = (block_pos & 1) ? byte >> 4 : byte & 15;
        s32 step = steps[step_index];
        s32 diff = step >> 3;
        if (nib & 4)
            diff += step;
        if (nib & 2)
            diff += step >> 1;
        if (nib & 1)
            diff += step >> 2;
        if (nib & 8)
            predictor -= diff;
        else
            predictor += diff;
        if (predictor > 32767)
            predictor = 32767;
        if (predictor < -32768)
            predictor = -32768;
        step_index += index_table[nib];
        if (step_index < 0)
            step_index = 0;
        if (step_index > 88)
            step_index = 88;
        out[i] = (s8)(predictor >> 8);
        remaining--;
        if (++block_pos == 2048) {
            block_pos = 0;
            src += 1024;
        }
    }
}

void audio_start(const u8 *data, u32 samples)
{
    playing = 0;
    src = data;
    remaining = samples;
    block_pos = 0;
    predictor = 0;
    step_index = 0;
    clock = 0;
    cur = 0;
    fill(buffers[0]);
    REG_SOUNDCNT_X = 0x0080;
    REG_SOUNDCNT_L = 0;
    REG_SOUNDCNT_H = 0x0B04;
    REG_TM0CNT_H = 0;
    REG_TM0CNT_L = 65536 - 1254;
    REG_TM0CNT_H = 0x0080;
    playing = 1;
}

void audio_pause(void)
{
    playing = 0;
    REG_DMA1CNT_H = 0;
    REG_TM0CNT_H = 0;
    REG_SOUNDCNT_H |= 0x0800;
}

void audio_resume(void)
{
    REG_TM0CNT_H = 0x0080;
    playing = 1;
}

void audio_stop(void)
{
    audio_pause();
}

u32 audio_clock(void)
{
    return clock;
}

void audio_vblank(void)
{
    if (!playing)
        return;
    REG_DMA1CNT_H = 0;
    REG_DMA1SAD = (u32)buffers[cur];
    REG_DMA1DAD = FIFO_A_ADDR;
    REG_DMA1CNT_H = 0xB640;
    clock += FRAME_SAMPLES;
    cur ^= 1;
    fill(buffers[cur]);
}

void audio_seek(const u8 *data, u32 total, u32 sample, int paused)
{
    playing = 0;
    REG_DMA1CNT_H = 0;
    REG_TM0CNT_H = 0;
    u32 block = sample >> 11;
    u32 start = block << 11;
    if (start > total)
        start = total;
    src = data + block * 1028;
    remaining = total - start;
    block_pos = 0;
    predictor = 0;
    step_index = 0;
    clock = start;
    cur = 0;
    REG_SOUNDCNT_H |= 0x0800;
    fill(buffers[0]);
    if (!paused) {
        REG_TM0CNT_H = 0x0080;
        playing = 1;
    }
}
