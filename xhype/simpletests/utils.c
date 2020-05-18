#include <stdio.h>
#include <stdarg.h>

void print_green(char* msg, ...) {
  fprintf(stderr, "\033[32m");
  va_list argp;

  va_start(argp, msg);
  vfprintf(stderr, msg, argp);
  va_end(argp);
  fprintf(stderr, "\033[0m");
}

void print_red(char* msg, ...) {
  fprintf(stderr, "\033[0;31m");
  va_list argp;

  va_start(argp, msg);
  vfprintf(stderr, msg, argp);
  va_end(argp);
  fprintf(stderr, "\033[0m");
}