"""Guard S3 LCD completion and DMA waveform contracts."""
from pathlib import Path
import subprocess
import tempfile
import unittest

ROOT = Path(__file__).resolve().parents[1]
SOURCE = ROOT / 'firmware/components/parallel_lcd/parallel_lcd.c'
INCLUDE = ROOT / 'firmware/components/parallel_lcd/include'

class LcdCompletionTests(unittest.TestCase):
    def test_poll_raw_completion_when_cpu_interrupts_are_disabled(self):
        source = SOURCE.read_text()
        self.assertIn('lcd_ll_enable_interrupt(p->hal.dev, LCD_LL_EVENT_TRANS_DONE, false)', source)
        self.assertIn('p->hal.dev->lc_dma_int_raw.val & LCD_LL_EVENT_TRANS_DONE', source)
        self.assertNotIn('lcd_ll_get_interrupt_status(p->hal.dev)', source)

    def test_descriptor_stream_matches_reference_encoder(self):
        source = SOURCE.read_text()
        start = source.index('// BEGIN HOST-TESTED DMA HELPERS')
        end = source.index('// END HOST-TESTED DMA HELPERS', start)
        helpers = source[start:end]
        harness = r'''
#include <assert.h>
#include <stdint.h>
#include <stdlib.h>
#include <string.h>
#include "lcd_waveform.h"
#define DMA_DESCRIPTOR_BUFFER_MAX_SIZE 4095u
#define DMA_DESCRIPTOR_BUFFER_OWNER_DMA 1u
typedef struct dma_descriptor_s dma_descriptor_t;
struct dma_descriptor_s {
    struct { unsigned size, length, suc_eof, owner; } dw0;
    void *buffer;
    dma_descriptor_t *next;
};
''' + helpers + r'''
int main(void) {
    const size_t sizes[] = {1, 31, 32, 51, 52, 768, 769};
    for (size_t k = 0; k < sizeof(sizes)/sizeof(sizes[0]); ++k) {
        size_t n = sizes[k], data_len = n * LCD_SAMPLES_PER_BYTE;
        uint8_t *lanes = malloc(4*n), *expected = malloc(data_len + LCD_RESET_SAMPLES);
        uint8_t *wave = calloc(1, data_len + LCD_RESET_BLOCK_SIZE);
        size_t count = lcd_dma_node_count(data_len);
        dma_descriptor_t *nodes = calloc(count, sizeof(*nodes));
        for (size_t i = 0; i < 4*n; ++i) lanes[i] = (i * 71u + k * 31u) & 255u;
        assert(lcd_encode(expected, data_len + LCD_RESET_SAMPLES, lanes, n) == data_len + LCD_RESET_SAMPLES);
        lcd_encode_data(wave, lanes, n);
        lcd_mount_dma(nodes, count, wave, data_len);
        size_t pos = 0, reset_nodes = 0;
        for (size_t i = 0; i < count; ++i) {
            dma_descriptor_t *d = &nodes[i];
            assert(d->dw0.owner == DMA_DESCRIPTOR_BUFFER_OWNER_DMA);
            assert(d->dw0.length && d->dw0.length <= DMA_DESCRIPTOR_BUFFER_MAX_SIZE);
            assert(d->dw0.size == d->dw0.length);
            assert(d->dw0.suc_eof == (i == count - 1));
            assert(d->next == (i + 1 == count ? NULL : &nodes[i+1]));
            if (pos >= data_len) {
                assert(d->buffer == wave + data_len);
                assert(d->dw0.length <= LCD_RESET_BLOCK_SIZE);
                ++reset_nodes;
            } else assert(pos + d->dw0.length <= data_len);
            assert(pos + d->dw0.length <= data_len + LCD_RESET_SAMPLES);
            assert(memcmp(d->buffer, expected + pos, d->dw0.length) == 0);
            pos += d->dw0.length;
        }
        assert(pos == data_len + LCD_RESET_SAMPLES);
        assert(reset_nodes > 1);
        assert(LCD_RESET_BLOCK_SIZE < LCD_RESET_SAMPLES - 1024);
        free(nodes); free(wave); free(expected); free(lanes);
    }
}
'''
        with tempfile.TemporaryDirectory() as temp:
            c = Path(temp) / 'test.c'
            exe = Path(temp) / 'test'
            c.write_text(harness)
            subprocess.run(['cc', '-std=c11', '-Wall', '-Wextra', '-Werror',
                            '-I', str(INCLUDE), str(c), '-o', str(exe)], check=True)
            subprocess.run([str(exe)], check=True)

if __name__ == '__main__':
    unittest.main()
