#include <assert.h>
#include <stdint.h>
#include <stdlib.h>
#include <string.h>
#include "lcd_waveform.h"

// Independent expected waveform: 125 ns samples, 375/750 ns HIGH,
// 1250 ns total bit period, followed by 320 us LOW.
static void check(const uint8_t *lanes, size_t bytes, uint8_t *wave, size_t capacity) {
    size_t data_samples = bytes * 8u * 10u;
    assert(lcd_encode(wave, capacity, lanes, bytes) == data_samples + 2560u);
    for (size_t lane = 0; lane < 4; lane++) {
        for (size_t byte = 0; byte < bytes; byte++) {
            uint8_t decoded = 0;
            for (unsigned bit = 0; bit < 8; bit++) {
                unsigned one = (lanes[lane * bytes + byte] >> (7 - bit)) & 1u;
                unsigned high = one ? 6 : 3;
                size_t start = (byte * 8u + bit) * 10u;
                for (unsigned tick = 0; tick < 10; tick++) {
                    assert(((wave[start + tick] >> lane) & 1u) == (tick < high));
                    assert((wave[start + tick] & 0xf0u) == 0);
                }
                decoded = (decoded << 1) | ((wave[start + 4] >> lane) & 1u);
            }
            assert(decoded == lanes[lane * bytes + byte]);
        }
    }
    for (size_t tick = data_samples; tick < data_samples + 2560u; tick++)
        assert(wave[tick] == 0);
}

int main(void) {
    assert(LCD_SAMPLES_PER_BYTE == 80u);
    assert(LCD_RESET_SAMPLES == 2560u);
    assert(LCD_SAMPLE_HZ == 8000000u);
    uint8_t lanes[4] = {0x80, 0x00, 0xff, 0x01};
    uint8_t short_wave[80 + 2560];
    memset(short_wave, 0xff, sizeof(short_wave));
    check(lanes, 1, short_wave, sizeof(short_wave));
    assert(lcd_encode(short_wave, sizeof(short_wave) - 1, lanes, 1) == 0);
    assert(lcd_encode(short_wave, sizeof(short_wave), lanes, SIZE_MAX) == 0);
    assert(lcd_encode(NULL, sizeof(short_wave), lanes, 1) == 0);
    assert(lcd_encode(short_wave, sizeof(short_wave), NULL, 1) == 0);
    uint8_t *input = malloc(4 * 768);
    uint8_t *wave = malloc(64000);
    assert(input && wave);
    srand(41);
    for (unsigned frame = 0; frame < 200; frame++) {
        for (size_t i = 0; i < 4 * 768; i++) input[i] = (uint8_t)rand();
        check(input, 768, wave, 64000);
    }
    free(input);
    free(wave);
    return 0;
}
