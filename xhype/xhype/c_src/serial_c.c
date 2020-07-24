/* SPDX-License-Identifier: GPL-2.0-only */

#include <termios.h>
#include <unistd.h>
#include <stdlib.h>

void make_stdin_raw_c() {
    struct termios old;
    tcgetattr(STDIN_FILENO, &old);
    struct termios new = old;
    cfmakeraw(&new);
    new.c_cflag |= CLOCAL;
    tcsetattr(STDIN_FILENO, TCSANOW, &new);
}