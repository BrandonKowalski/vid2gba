#ifndef MENU_H
#define MENU_H

#include "gba.h"

void menu_init(const u8 *data, u32 tiles);
void menu_show(const u8 *labels, int count, int selected);
void menu_hide(void);
void menu_vblank(void);

#endif
