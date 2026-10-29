#pragma once
#include <stddef.h>
#include <stdint.h>

// 3.2 MHz: 312.5 ns per sample; reset is 320 us.
#define LCD_RESET_SAMPLES 1024u
#define LCD_SAMPLES_PER_BYTE 32u

// Input is four contiguous lanes, each lane_bytes bytes, in GRB order.
static inline size_t lcd_encode(uint8_t *out, size_t capacity,
                                const uint8_t *lanes, size_t lane_bytes) {
    if (!out || !lanes || lane_bytes > (SIZE_MAX - LCD_RESET_SAMPLES) / LCD_SAMPLES_PER_BYTE)
        return 0;
    size_t n = lane_bytes * LCD_SAMPLES_PER_BYTE + LCD_RESET_SAMPLES;
    if (capacity < n) return 0;
    size_t pos = 0;
    for (size_t byte = 0; byte < lane_bytes; byte++) {
        for (int bit = 7; bit >= 0; bit--) {
            uint8_t ones = 0;
            for (size_t lane = 0; lane < 4; lane++)
                ones |= ((lanes[lane * lane_bytes + byte] >> bit) & 1u) << lane;
            out[pos++] = 15;
            out[pos++] = ones;
            out[pos++] = ones;
            out[pos++] = 0;
        }
    }
    while (pos < n) out[pos++] = 0;
    return n;
}
