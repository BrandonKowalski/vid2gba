    .section .crt0, "ax", %progbits
    .arm
    .global _start
_start:
    b       start
    .fill   156, 1, 0
    .ascii  "VID2GBA     "
    .ascii  "VGBA"
    .ascii  "00"
    .byte   0x96
    .byte   0
    .byte   0
    .fill   7, 1, 0
    .byte   0
    .byte   0
    .fill   2, 1, 0
start:
    mov     r0, #0x12
    msr     cpsr_c, r0
    ldr     sp, =0x03007FA0
    mov     r0, #0x1F
    msr     cpsr_c, r0
    ldr     sp, =0x03007F00
    ldr     r0, =__iwram_lma
    ldr     r1, =__iwram_start
    ldr     r2, =__iwram_end
1:  cmp     r1, r2
    ldrlo   r3, [r0], #4
    strlo   r3, [r1], #4
    blo     1b
    ldr     r1, =__bss_start
    ldr     r2, =__bss_end
    mov     r3, #0
2:  cmp     r1, r2
    strlo   r3, [r1], #4
    blo     2b
    ldr     r0, =irq_handler
    ldr     r1, =0x03007FFC
    str     r0, [r1]
    ldr     r0, =main
    mov     lr, pc
    bx      r0
3:  b       3b
    .ltorg

    .section .iwram.text, "ax", %progbits
    .arm
    .global irq_handler
irq_handler:
    mov     r2, #0x04000000
    add     r2, r2, #0x200
    ldr     r1, [r2]
    and     r0, r1, r1, lsr #16
    strh    r0, [r2, #2]
    ldr     r3, =0x03007FF8
    ldrh    r1, [r3]
    orr     r1, r1, r0
    strh    r1, [r3]
    stmfd   sp!, {lr}
    ldr     r1, =isr
    mov     lr, pc
    bx      r1
    ldmfd   sp!, {lr}
    bx      lr
    .ltorg

    .section .text, "ax", %progbits
    .thumb
    .irp    reg, r0, r1, r2, r3, r4, r5, r6, r7
    .global _call_via_\reg
    .thumb_func
_call_via_\reg:
    bx      \reg
    .endr
