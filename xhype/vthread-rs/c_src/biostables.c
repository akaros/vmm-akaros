// from Akaros biostables.c
#include <stdint.h>

uint8_t acpi_tb_checksum_c(uint8_t *buffer, uint32_t length) {
  uint8_t sum = 0;
  uint8_t *end = buffer + length;

  while (buffer < end) sum = (uint8_t)(sum + *(buffer++));
  return sum;
}