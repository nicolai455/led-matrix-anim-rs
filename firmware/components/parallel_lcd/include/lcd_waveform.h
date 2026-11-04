#pragma once
#include <stddef.h>
#include <stdint.h>

// 8 MHz: 125 ns per sample. Ten samples preserve the 800 kbit/s
// wire rate, with zero=1110000000 and one=1111110000.
#define LCD_SAMPLE_HZ 8000000u
#define LCD_SAMPLES_PER_BIT 10u
#define LCD_ZERO_HIGH_SAMPLES 3u
#define LCD_ONE_HIGH_SAMPLES 6u
#define LCD_RESET_SAMPLES (LCD_SAMPLE_HZ / 1000000u * 320u)
#define LCD_SAMPLES_PER_BYTE (8u * LCD_SAMPLES_PER_BIT)

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
            for (unsigned tick = 0; tick < LCD_SAMPLES_PER_BIT; tick++)
                out[pos++] = tick < LCD_ZERO_HIGH_SAMPLES ? 15u
                           : tick < LCD_ONE_HIGH_SAMPLES ? ones : 0u;
        }
    }
    while (pos < n) out[pos++] = 0;
    return n;
}
