#include "sdkconfig.h"
#if CONFIG_IDF_TARGET_ESP32S3
#include <stdlib.h>
#include <string.h>
#include "lcd_waveform.h"
#include "driver/gpio.h"
#include "esp_heap_caps.h"
#include "esp_private/gdma.h"
#include "esp_private/periph_ctrl.h"
#include "esp_rom_gpio.h"
#include "esp_rom_sys.h"
#include "esp_timer.h"
#include "esp_pm.h"
#include "hal/lcd_hal.h"
#include "hal/gpio_hal.h"
#include "soc/io_mux_reg.h"
#include "hal/lcd_ll.h"
#include "hal/dma_types.h"
#include "soc/lcd_periph.h"
#include "soc/gpio_sig_map.h"
#include "freertos/FreeRTOS.h"
#include "freertos/task.h"

// Private IDF platform registry prevents conflict with the public LCD driver.
extern int lcd_com_register_device(int type, void *object);
extern void lcd_com_remove_device(int type, int id);
extern void lcd_com_mount_dma_data(dma_descriptor_t *, const void *, size_t);

typedef struct {
    lcd_hal_context_t hal;
    gdma_channel_handle_t dma;
    esp_pm_lock_handle_t pm;
    dma_descriptor_t *nodes;
    uint8_t *wave;
    size_t wave_len, node_count;
    int id, pins[4], routed;
    bool clock_on, connected, aborted;
} parallel_lcd_t;

void parallel_lcd_destroy(void *ctx) {
    parallel_lcd_t *p = ctx;
    if (!p) return;
    if (p->dma) { gdma_stop(p->dma); gdma_reset(p->dma); }
    if (p->clock_on) { lcd_ll_stop(p->hal.dev); lcd_ll_fifo_reset(p->hal.dev); }
    for (int i = 0; i < p->routed; i++) {
        esp_rom_gpio_connect_out_signal(p->pins[i], SIG_GPIO_OUT_IDX, false, false);
        gpio_set_level(p->pins[i], 0);
    }
    if (p->dma) {
        if (p->connected) gdma_disconnect(p->dma);
        gdma_del_channel(p->dma);
    }
    if (p->clock_on) {
        lcd_ll_enable_clock(p->hal.dev, false);
        PERIPH_RCC_RELEASE_ATOMIC(lcd_periph_signals.panels[p->id].module, ref_count) {
            if (ref_count == 0) lcd_ll_enable_bus_clock(p->id, false);
        }
    }
    if (p->id >= 0) lcd_com_remove_device(0, p->id);
    if (p->pm) esp_pm_lock_delete(p->pm);
    free(p->nodes); free(p->wave); free(p);
}

void *parallel_lcd_create(const int *pins, size_t lane_bytes) {
    if (!pins || !lane_bytes || lane_bytes > (SIZE_MAX - LCD_RESET_SAMPLES) / 32) return NULL;
    for (int i = 0; i < 4; i++) {
        if (!GPIO_IS_VALID_OUTPUT_GPIO(pins[i])) return NULL;
        for (int j = 0; j < i; j++) if (pins[i] == pins[j]) return NULL;
    }
    parallel_lcd_t *p = calloc(1, sizeof(*p));
    if (!p) return NULL;
    p->id = -1;
    p->wave_len = lane_bytes * 32 + LCD_RESET_SAMPLES;
    p->node_count = (p->wave_len + DMA_DESCRIPTOR_BUFFER_MAX_SIZE - 1) / DMA_DESCRIPTOR_BUFFER_MAX_SIZE;
    p->wave = heap_caps_calloc(1, p->wave_len, MALLOC_CAP_INTERNAL | MALLOC_CAP_DMA);
    p->nodes = heap_caps_calloc(p->node_count, sizeof(*p->nodes), MALLOC_CAP_INTERNAL | MALLOC_CAP_DMA);
    if (!p->wave || !p->nodes) goto fail;
    for (size_t i = 0; i + 1 < p->node_count; i++) p->nodes[i].next = &p->nodes[i + 1];
    p->id = lcd_com_register_device(0, p);
    if (p->id < 0) goto fail;
    PERIPH_RCC_ACQUIRE_ATOMIC(lcd_periph_signals.panels[p->id].module, ref_count) {
        if (ref_count == 0) { lcd_ll_enable_bus_clock(p->id, true); lcd_ll_reset_register(p->id); }
    }
    lcd_hal_init(&p->hal, p->id); p->clock_on = true;
    lcd_ll_reset(p->hal.dev); lcd_ll_fifo_reset(p->hal.dev);
    lcd_ll_enable_clock(p->hal.dev, true);
    lcd_ll_select_clk_src(p->hal.dev, LCD_CLK_SRC_PLL160M);
    lcd_ll_set_group_clock_coeff(p->hal.dev, 2, 0, 0);
    lcd_ll_set_pixel_clock_prescale(p->hal.dev, 25); // 160 MHz / 2 / 25 = 3.2 MHz
    lcd_ll_enable_rgb_mode(p->hal.dev, false);
    lcd_ll_set_data_width(p->hal.dev, 8);
    lcd_ll_enable_output_always_on(p->hal.dev, true);
    lcd_ll_reverse_bit_order(p->hal.dev, false);
    lcd_ll_swap_byte_order(p->hal.dev, 8, false);
    lcd_ll_set_phase_cycles(p->hal.dev, 0, 0, 1);
    lcd_ll_set_blank_cycles(p->hal.dev, 1, 1);
    lcd_ll_enable_interrupt(p->hal.dev, LCD_LL_EVENT_TRANS_DONE, false);
    gdma_channel_alloc_config_t config = { .direction = GDMA_CHANNEL_DIRECTION_TX };
    if (gdma_new_channel(&config, &p->dma) != ESP_OK) goto fail;
    if (gdma_connect(p->dma, GDMA_MAKE_TRIGGER(GDMA_TRIG_PERIPH_LCD, 0)) != ESP_OK) goto fail;
    p->connected = true;
    gdma_strategy_config_t strategy = { .owner_check = false, .auto_update_desc = false };
    if (gdma_apply_strategy(p->dma, &strategy) != ESP_OK) goto fail;
#if CONFIG_PM_ENABLE
    if (esp_pm_lock_create(ESP_PM_NO_LIGHT_SLEEP, 0, "parallel_lcd", &p->pm) != ESP_OK) goto fail;
#endif
    for (int i = 0; i < 4; i++) {
        p->pins[i] = pins[i];
        if (gpio_set_direction(pins[i], GPIO_MODE_OUTPUT) != ESP_OK) goto fail;
        gpio_set_level(pins[i], 0);
        esp_rom_gpio_connect_out_signal(pins[i], lcd_periph_signals.buses[p->id].data_sigs[i], false, false);
        gpio_hal_iomux_func_sel(GPIO_PIN_MUX_REG[pins[i]], PIN_FUNC_GPIO);
        p->routed++;
    }
    return p;
fail:
    parallel_lcd_destroy(p); return NULL;
}

int parallel_lcd_send(void *ctx, const uint8_t *lanes, size_t lane_bytes) {
    parallel_lcd_t *p = ctx;
    if (!p || !lanes || lane_bytes != (p->wave_len - LCD_RESET_SAMPLES) / 32) return ESP_ERR_INVALID_ARG;
    if (p->aborted) return ESP_ERR_INVALID_STATE; // require reinitialization after an incomplete frame
    if (!lcd_encode(p->wave, p->wave_len, lanes, lane_bytes)) return ESP_FAIL;
    if (p->pm && esp_pm_lock_acquire(p->pm) != ESP_OK) return ESP_FAIL;
    lcd_ll_stop(p->hal.dev);
    gdma_reset(p->dma);
    lcd_ll_fifo_reset(p->hal.dev);
    lcd_ll_clear_interrupt_status(p->hal.dev, UINT32_MAX);
    lcd_com_mount_dma_data(p->nodes, p->wave, p->wave_len);
    esp_err_t err = gdma_start(p->dma, (intptr_t)p->nodes);
    if (err == ESP_OK) {
        esp_rom_delay_us(1);
        lcd_ll_start(p->hal.dev);
        int64_t deadline = esp_timer_get_time() + 1000000;
        while (!(lcd_ll_get_interrupt_status(p->hal.dev) & LCD_LL_EVENT_TRANS_DONE)) {
            if (esp_timer_get_time() >= deadline) { err = ESP_ERR_TIMEOUT; break; }
            vTaskDelay(1);
        }
    }
    // Stop both engines before the owned DMA payload can be reused or freed.
    lcd_ll_stop(p->hal.dev); gdma_stop(p->dma); gdma_reset(p->dma);
    if (err != ESP_OK) {
        p->aborted = true;
        for (int i = 0; i < p->routed; i++) {
            gpio_set_level(p->pins[i], 0);
            esp_rom_gpio_connect_out_signal(p->pins[i], SIG_GPIO_OUT_IDX, false, false);
        }
    }
    if (p->pm) esp_pm_lock_release(p->pm);
    return err;
}
#endif
