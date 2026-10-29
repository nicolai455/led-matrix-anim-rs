#include <assert.h>
#include <stdint.h>
#include <string.h>
#include "lcd_waveform.h"
int main(void) {
    uint8_t lanes[4] = {0x80, 0x00, 0xff, 0x01};
    uint8_t out[8 * 4 + LCD_RESET_SAMPLES];
    memset(out, 0xff, sizeof(out));
    assert(lcd_encode(out, sizeof(out), lanes, 1) == sizeof(out));
    assert(out[0] == 15 && out[1] == 5 && out[2] == 5 && out[3] == 0);
    assert(out[28] == 15 && out[29] == 12 && out[30] == 12 && out[31] == 0);
    for (unsigned i = 32; i < sizeof(out); i++) assert(out[i] == 0);
    assert(lcd_encode(out, sizeof(out)-1, lanes, 1) == 0);
    assert(lcd_encode(out, sizeof(out), lanes, SIZE_MAX) == 0);
    uint8_t multi[] = {0x12,0x34, 0x56,0x78, 0x9a,0xbc, 0xde,0xf0};
    uint8_t expanded[2 * 32 + LCD_RESET_SAMPLES];
    assert(lcd_encode(expanded, sizeof(expanded), multi, 2) == sizeof(expanded));
    for (unsigned byte = 0; byte < 2; byte++) {
        for (unsigned bit = 0; bit < 8; bit++) {
            unsigned mask = 0;
            for (unsigned lane = 0; lane < 4; lane++)
                mask |= ((multi[lane * 2 + byte] >> (7 - bit)) & 1u) << lane;
            unsigned p = byte * 32 + bit * 4;
            assert(expanded[p] == 15 && expanded[p + 1] == mask);
            assert(expanded[p + 2] == mask && expanded[p + 3] == 0);
        }
    }
    for (unsigned i = 64; i < sizeof(expanded); i++) assert(expanded[i] == 0);
    return 0;
}
