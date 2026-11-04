"""Guard the S3 completion polling contract against masked-status timeouts."""
from pathlib import Path
import unittest
ROOT = Path(__file__).resolve().parents[1]

class LcdCompletionTests(unittest.TestCase):
    def test_poll_raw_completion_when_cpu_interrupts_are_disabled(self):
        source = (ROOT / 'firmware/components/parallel_lcd/parallel_lcd.c').read_text()
        self.assertIn('lcd_ll_enable_interrupt(p->hal.dev, LCD_LL_EVENT_TRANS_DONE, false)', source)
        self.assertIn('p->hal.dev->lc_dma_int_raw.val & LCD_LL_EVENT_TRANS_DONE', source)
        self.assertNotIn('lcd_ll_get_interrupt_status(p->hal.dev)', source)

    def test_waveform_allocation_tracks_encoder_samples_per_byte(self):
        source = (ROOT / 'firmware/components/parallel_lcd/parallel_lcd.c').read_text()
        self.assertIn('lane_bytes * LCD_SAMPLES_PER_BYTE + LCD_RESET_SAMPLES', source)
        self.assertNotIn('lane_bytes * 32', source)
        self.assertNotIn('(p->wave_len - LCD_RESET_SAMPLES) / 32', source)

if __name__ == '__main__':
    unittest.main()
