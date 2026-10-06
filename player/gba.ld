OUTPUT_FORMAT("elf32-littlearm")
OUTPUT_ARCH(arm)
ENTRY(_start)

MEMORY
{
    rom   : ORIGIN = 0x08000000, LENGTH = 32M
    iwram : ORIGIN = 0x03000000, LENGTH = 0x7000
}

SECTIONS
{
    .text :
    {
        KEEP(*(.crt0))
        *(.text .text.*)
        *(.glue_7 .glue_7t .v4_bx)
        *(.rodata .rodata.*)
        . = ALIGN(4);
    } > rom

    .iwram :
    {
        __iwram_start = .;
        *(.iwram.text)
        *(.data .data.*)
        . = ALIGN(4);
        __iwram_end = .;
    } > iwram AT > rom

    __iwram_lma = LOADADDR(.iwram);

    .bss (NOLOAD) :
    {
        __bss_start = .;
        *(.bss .bss.*)
        *(COMMON)
        . = ALIGN(4);
        __bss_end = .;
    } > iwram

    /DISCARD/ : { *(.ARM.exidx*) *(.comment) }
}
