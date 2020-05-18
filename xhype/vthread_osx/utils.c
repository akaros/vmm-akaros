#include <ctype.h>
#include <stdarg.h>
#include <stdint.h>
#include <stdio.h>

void print_green(char *msg, ...) {
  fprintf(stdout, "\033[32m");
  va_list argp;

  va_start(argp, msg);
  vfprintf(stdout, msg, argp);
  va_end(argp);
  fprintf(stdout, "\033[0m");
}

void print_red(char *msg, ...) {
  fprintf(stdout, "\033[0;31m");
  va_list argp;

  va_start(argp, msg);
  vfprintf(stdout, msg, argp);
  va_end(argp);
  fprintf(stdout, "\033[0m");
}

void print_hex_ascii_line(const uint8_t *payload, int len, int offset) {
  int i;
  int gap;
  const uint8_t *ch;

  /* offset */
  printf("%05d   ", offset);

  /* hex */
  ch = payload;
  for (i = 0; i < len; i++) {
    printf("%02x ", *ch);
    ch++;
    /* print extra space after 8th byte for visual aid */
    if (i == 7) printf(" ");
  }
  /* print space to handle line less than 8 bytes */
  if (len < 8) printf(" ");

  /* fill hex gap with spaces if not full line */
  if (len < 16) {
    gap = 16 - len;
    for (i = 0; i < gap; i++) {
      printf("   ");
    }
  }
  printf("   ");

  /* ascii (if printable) */
  ch = payload;
  for (i = 0; i < len; i++) {
    if (isprint(*ch))
      printf("%c", *ch);
    else
      printf(".");
    ch++;
  }

  printf("\n");

  return;
}

void print_payload(const void *payload, int len) {
  int len_rem = len;
  int line_width = 16; /* number of bytes per line */
  int line_len;
  int offset = 0; /* zero-based offset counter */
  const uint8_t *ch = (uint8_t *)payload;

  if (len <= 0) return;

  /* data fits on one line */
  if (len <= line_width) {
    print_hex_ascii_line(ch, len, offset);
    return;
  }

  /* data spans multiple lines */
  for (;;) {
    /* compute current line length */
    line_len = line_width % len_rem;
    /* print line */
    print_hex_ascii_line(ch, line_len, offset);
    /* compute total remaining */
    len_rem = len_rem - line_len;
    /* shift pointer to remaining bytes to print */
    ch = ch + line_len;
    /* add offset */
    offset = offset + line_width;
    /* check if we have line width chars or less */
    if (len_rem <= line_width) {
      /* print last line and get out */
      print_hex_ascii_line(ch, len_rem, offset);
      break;
    }
  }

  return;
}
