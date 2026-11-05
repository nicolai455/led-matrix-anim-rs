# ESP32-S3 LCD/GDMA three-block streaming feasibility

**Status: research/design only; not safe to integrate or flash.** A circular
three-block transmitter is plausible, but the IDF API alone does not establish
that a block is safe to rewrite after its EOF callback, or that deadlines can
always be met. This is a *proposed* triple ring, not a claim that WLED uses
three blocks. [WLED PR 5704](https://github.com/wled/WLED/pull/5704) describes
ping-pong DMA for parallel outputs; its approach motivates the experiment but
is not proof of the hardware properties below.

## Baseline and API evidence

The repository build script identifies ESP-IDF 5.2.1; the exact embedded IDF
checkout was not available for inspection in this workspace. The analysis
uses upstream **v5.2.1**, which should be compared against the actual built
checkout before any target work. The existing S3 driver uses an 8-bit LCD bus,
8 MHz sample clock, 10 samples/bit and four packed output lanes. Each input
lane byte expands to 80 DMA bytes. Its static data descriptors are followed by
reused zero blocks totaling 2,560 samples (320 µs), and it waits for the LCD
raw transfer-done event, not GDMA EOF, before stopping both engines. Its
one-second timeout fails closed and routes output low. Do not change this
path as part of the experiment.

IDF's [`esp_private/gdma.h`](https://github.com/espressif/esp-idf/blob/v5.2.1/components/esp_hw_support/include/esp_private/gdma.h)
is explicitly private. It permits `gdma_start` on a linked list or ring and
`gdma_register_tx_event_callbacks`, with `on_trans_eof`, `on_descr_err`, and
`gdma_event_data_t.tx_eof_desc_addr`. `gdma_apply_strategy` controls
`owner_check` and TX `auto_update_desc`; their names alone do not prove when
payload ownership returns. `gdma_append` wakes/appends a paused chain; it is
not documented as a safe live-descriptor mutation barrier. Start, stop, reset,
and append are documented ISR-callable; cache-disabled safety requires
`CONFIG_GDMA_CTRL_FUNC_IN_IRAM`. With `CONFIG_GDMA_ISR_IRAM_SAFE`, callback
code and callback context must satisfy the registration checks, and all code,
constants, data and synchronization used in the callback must be accessible
with flash cache disabled. Inspect the actual Kconfig/build and entire call
chain, not just an `IRAM_ATTR` annotation.

In the [v5.2.1 implementation](https://github.com/espressif/esp-idf/blob/v5.2.1/components/esp_hw_support/dma/gdma.c),
the TX ISR reads and clears an interrupt status bit then reads one EOF
descriptor address and invokes the callback. It does not expose an EOF event
queue or generation counter. Multiple EOFs while the interrupt is masked or
unserviced may therefore be indistinguishable; confirm on hardware. The EOF
address is not a wire-completion notification: LCD FIFO/prefetch may still
contain data. Neither a callback count nor that address proves that GDMA cannot
already have fetched the descriptor or payload of a later block.

## Timing budget and memory

At 8 MHz one output byte occupies 125 ns. A 1,024-byte block lasts 128 µs;
three blocks occupy 3,072 DMA payload bytes, plus aligned descriptors, source
snapshot, zero/reset handling and control data. Nominally, after EOF of one
block, two blocks contain 256 µs of output before its next turn, but **this is
not a guaranteed CPU refill window**: interrupt latency, GDMA/LCD FIFO and
prefetch reduce it, and there may be missed/coalesced EOFs. A 4,092-byte block
lasts 511.5 µs; check the actual descriptor limits, alignment and allocation
against the built target. Capture worst-case latency and refill time under
Wi-Fi, flash/cache-disabled operations and other interrupts; require a measured,
conservatively justified margin, not an average. Nominal pulse timing is 375 ns
high for zero, 750 ns high for one, 1.25 µs per bit; verify actual GPIO output
and LED datasheet tolerances with a logic analyzer.

## Blocking safety questions

1. **Reuse and prefetch:** establish from S3 TRM/errata and measurements when
   GDMA has finished reading a block and how far it may fetch ahead; determine
   whether circular links and owner/update modes permit bounded safe reuse.
   A release/acquire atomic or compiler barrier alone cannot prove device DMA
   visibility or ownership. Do not patch a live `next` pointer based on an
   assumed prefetch depth.
2. **Lost progress/deadline:** a single EOF address may replace earlier events;
   after delayed ISR service the ring could already be reading a buffer while
   CPU writes it. A callback-only sequence check cannot prevent that race.
   Need a target-proven pre-write guard that fails closed *before any write*
   when ownership or margin is uncertain, including wraparound and ABA cases.
   If no such guard exists with these APIs, reject continuous live refill.
3. **Finite termination and reset:** avoid replaying stale data after the last
   partial block. Define exact zero padding and transmit at least 2,560 zeros,
   then wait for actual LCD wire completion before buffer reuse or success.
   A dynamic ring-to-tail switch requires the same unresolved prefetch proof.
   EOF itself is insufficient. Timeout/error must stop both engines, drive
   output low, invalidate the context and not free memory until DMA is quiescent.
4. **Execution context:** ISR refill might be too slow; task refill adds
   scheduling latency. Source data must be immutable for the whole transfer,
   safely available when cache is disabled, and bounded; IRAM/DRAM budget,
   cross-core publication, DMA memory capability and barriers need target
   verification. Never allocate, log, block, or call non-IRAM code in the ISR.
5. **Resource and lifecycle:** preserve peripheral registration, PM clock lock,
   reset/abort/reinit semantics, descriptor errors, concurrent sends and
   destruction safety. Establish actual waveform RAM savings against the
   existing full-frame allocation, including source snapshots and all overhead.

## Isolated experiment / acceptance plan

No implementation is provided: a host simulation cannot establish the critical
hardware ownership and prefetch facts, so labeling a live refill prototype
safe would be misleading. If the hardware questions are resolved, build a
separate host-only model outside the production component before any firmware:

* Model three slots with monotonic generation, explicit CPU-writing, published,
  DMA-readable and retired states; simulate DMA prefetch separately from wire
  output. Fail on any write overlapping a possible DMA read. Inject delayed,
  duplicate, dropped and coalesced EOFs, stale generation/ABA, out-of-order
  events, wraparound, timeout, descriptor error and abort at every transition.
  Rejection must occur before writes, not merely detect corruption afterward.
* Compare concatenated modeled wire bytes with `lcd_encode` for randomized
  four-lane input, zero-length/short/unaligned/long inputs, all byte values,
  boundaries around each block and final reset; check bounds and arithmetic
  overflow with sanitizers. Prove output is exactly the expected frame then
  the 2,560 low samples and that send cannot report success early.
* Only after the model passes and target ownership semantics are justified,
  make a standalone opt-in bench firmware, with no production integration or
  automatic flashing. Instrument timestamps and GPIO markers, measure EOF to
  last bus activity, refill latency, prefetch distance and interrupt loss under
  stress. Capture full data/reset and start/stop transitions; deliberately mask
  interrupts beyond one and two block intervals and inject errors. A pass
  requires safe abort without write/read overlap or false success, a complete
  low reset interval, and substantial documented worst-case timing margin.
  Test cache-disabled periods, Wi-Fi/flash activity, both cores, repeated
  start/stop and teardown. Host tests and a successful build do not substitute
  for these measurements.

If hardware cannot guarantee a pre-write ownership test and safe finite tail,
retain the current pre-encoded descriptor chain instead of shipping a ring.
