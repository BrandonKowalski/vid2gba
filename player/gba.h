#ifndef GBA_H
#define GBA_H

typedef unsigned char u8;
typedef unsigned short u16;
typedef unsigned int u32;
typedef unsigned long long u64;
typedef signed char s8;
typedef signed short s16;
typedef signed int s32;

#define REG16(a) (*(volatile u16 *)(a))
#define REG32(a) (*(volatile u32 *)(a))

#define REG_DISPCNT REG16(0x04000000)
#define REG_DISPSTAT REG16(0x04000004)
#define REG_BG2PA REG16(0x04000020)
#define REG_BG2PB REG16(0x04000022)
#define REG_BG2PC REG16(0x04000024)
#define REG_BG2PD REG16(0x04000026)
#define REG_BG2X REG32(0x04000028)
#define REG_BG2Y REG32(0x0400002C)
#define REG_BLDCNT REG16(0x04000050)
#define REG_BLDY REG16(0x04000054)
#define REG_SOUNDCNT_L REG16(0x04000080)
#define REG_SOUNDCNT_H REG16(0x04000082)
#define REG_SOUNDCNT_X REG16(0x04000084)
#define FIFO_A_ADDR 0x040000A0
#define REG_DMA1SAD REG32(0x040000BC)
#define REG_DMA1DAD REG32(0x040000C0)
#define REG_DMA1CNT_H REG16(0x040000C6)
#define REG_TM0CNT_L REG16(0x04000100)
#define REG_TM0CNT_H REG16(0x04000102)
#define REG_KEYINPUT REG16(0x04000130)
#define REG_IE REG16(0x04000200)
#define REG_WAITCNT REG16(0x04000204)
#define REG_IME REG16(0x04000208)

#define PAL_BG ((volatile u16 *)0x05000000)
#define VRAM ((volatile u16 *)0x06000000)
#define PAGE_U16 0x5000

#define KEY_A 0x0001
#define KEY_B 0x0002
#define KEY_SELECT 0x0004
#define KEY_START 0x0008
#define KEY_RIGHT 0x0010
#define KEY_LEFT 0x0020
#define KEY_UP 0x0040
#define KEY_DOWN 0x0080
#define KEY_R 0x0100
#define KEY_L 0x0200
#define IRQ_VBLANK 0x0001

#define IWRAM_CODE __attribute__((section(".iwram.text"), long_call, target("arm"), noinline))

static inline void vblank_wait(void)
{
    __asm__ volatile("swi 0x05" ::: "r0", "r1", "r2", "r3", "memory");
}

#endif
