CC := arm-none-eabi-gcc
OBJCOPY := arm-none-eabi-objcopy
CFLAGS := -mcpu=arm7tdmi -mthumb -mthumb-interwork -O2 -Wall -Wextra -ffreestanding -fno-builtin \
	-fno-tree-loop-distribute-patterns -fno-strict-aliasing
LDFLAGS := -nostdlib -nostartfiles -T gba.ld -Wl,--no-warn-rwx-segments
OBJS := crt0.o main.o video.o audio.o overlay.o menu.o subs.o

player.gba: player.elf
	$(OBJCOPY) -O binary $< $@

player.elf: $(OBJS) gba.ld
	$(CC) $(CFLAGS) $(LDFLAGS) -o $@ $(OBJS)

%.o: %.c gba.h video.h audio.h overlay.h menu.h subs.h
	$(CC) $(CFLAGS) -c -o $@ $<

%.o: %.s
	$(CC) $(CFLAGS) -c -o $@ $<

clean:
	rm -f *.o player.elf player.gba

.PHONY: clean
