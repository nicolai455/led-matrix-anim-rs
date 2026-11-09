use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq)]
pub struct AudioSnapshot {
    pub bands: [f32; 16],
    /// Audio waveform samples in signed range [-1.0, 1.0] (0.0 = silence).
    pub waveform: [f32; 128],
    pub level: f32,
    pub peak: f32,
    /// Normalized low-frequency energy (0.0 = silence, 1.0 = full scale).
    pub bass: f32,
    /// Normalized positive bass-energy novelty for this frame (0.0..=1.0).
    pub onset: f32,
    /// Estimated tempo in beats per minute; 0.0 means unavailable/stale.
    pub bpm: f32,
    /// One-frame onset pulse, derived by the shared core onset detector.
    pub beat: bool,
    pub available: bool,
}

impl Default for AudioSnapshot {
    fn default() -> Self {
        Self {
            bands: [0.0; 16],
            waveform: [0.0; 128],
            level: 0.0,
            peak: 0.0,
            bass: 0.0,
            onset: 0.0,
            bpm: 0.0,
            beat: false,
            available: false,
        }
    }
}

pub const AUDIO_STALE_AFTER_MS: u64 = 250;
pub const AUDIO_FADE_DURATION_MS: u64 = 500;

impl AudioSnapshot {
    pub fn silent() -> Self {
        Self::default()
    }

    /// Applies staleness and fade-out based on sample age in milliseconds.
    /// - age <= 250ms: sample is fresh and available.
    /// - 250ms < age < 750ms: sample is stale (`available = false`) and linearly faded.
    /// - age >= 750ms: sample is completely faded out to silence (`available = false`).
    pub fn apply_staleness(&mut self, age_ms: u64) {
        if age_ms <= AUDIO_STALE_AFTER_MS {
            return;
        }
        self.available = false;
        self.onset = 0.0;
        self.bpm = 0.0;
        self.beat = false;
        if age_ms >= AUDIO_STALE_AFTER_MS + AUDIO_FADE_DURATION_MS {
            self.level = 0.0;
            self.peak = 0.0;
            self.bass = 0.0;
            self.bands.fill(0.0);
            self.waveform.fill(0.0);
        } else {
            let fade_age = (age_ms - AUDIO_STALE_AFTER_MS) as f32;
            let gain = (1.0 - fade_age / (AUDIO_FADE_DURATION_MS as f32)).clamp(0.0, 1.0);
            self.level *= gain;
            self.peak *= gain;
            self.bass *= gain;
            for b in &mut self.bands {
                *b *= gain;
            }
            for w in &mut self.waveform {
                *w *= gain;
            }
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MicSettings {
    #[serde(
        default = "default_gain_db",
        deserialize_with = "deserialize_setting_float"
    )]
    pub gain_db: f32,
    #[serde(
        default = "default_squelch_db",
        deserialize_with = "deserialize_setting_float"
    )]
    pub squelch_db: f32,
    #[serde(
        default = "default_beat_threshold",
        deserialize_with = "deserialize_setting_float"
    )]
    pub beat_threshold: f32,
    #[serde(
        default = "default_attack_ms",
        deserialize_with = "deserialize_setting_float"
    )]
    pub attack_ms: f32,
    #[serde(
        default = "default_decay_ms",
        deserialize_with = "deserialize_setting_float"
    )]
    pub decay_ms: f32,
    #[serde(default = "default_agc")]
    pub agc: bool,
    #[serde(default = "default_right_channel")]
    pub right_channel: bool,
}

// Deserialize through the JSON-native f64 visitor before narrowing. This
// avoids the Xtensa LLVM f32 visitor constant-pool bug without disabling
// optimization for the surrounding networking and persistence crates.
fn deserialize_setting_float<'de, D: serde::Deserializer<'de>>(
    deserializer: D,
) -> Result<f32, D::Error> {
    let value = f64::deserialize(deserializer)?;
    let narrowed = value as f32;
    if !narrowed.is_finite() {
        return Err(serde::de::Error::custom("non-finite microphone setting"));
    }
    Ok(narrowed)
}

fn default_gain_db() -> f32 {
    30.0
}
fn default_squelch_db() -> f32 {
    -60.0
}
fn default_beat_threshold() -> f32 {
    1.6
}
fn default_attack_ms() -> f32 {
    40.0
}
fn default_decay_ms() -> f32 {
    300.0
}
fn default_agc() -> bool {
    true
}
fn default_right_channel() -> bool {
    false
}

impl Default for MicSettings {
    fn default() -> Self {
        Self {
            gain_db: default_gain_db(),
            squelch_db: default_squelch_db(),
            beat_threshold: default_beat_threshold(),
            attack_ms: default_attack_ms(),
            decay_ms: default_decay_ms(),
            agc: default_agc(),
            right_channel: default_right_channel(),
        }
    }
}

impl MicSettings {
    pub fn validate(&self) -> Result<(), &'static str> {
        if !self.gain_db.is_finite() || self.gain_db < 0.0 || self.gain_db > 60.0 {
            return Err("gain_db out of range [0, 60]");
        }
        if !self.squelch_db.is_finite() || self.squelch_db < -100.0 || self.squelch_db > -10.0 {
            return Err("squelch_db out of range [-100, -10]");
        }
        if !self.beat_threshold.is_finite()
            || self.beat_threshold < 1.0
            || self.beat_threshold > 5.0
        {
            return Err("beat_threshold out of range [1, 5]");
        }
        if !self.attack_ms.is_finite() || self.attack_ms < 1.0 || self.attack_ms > 1000.0 {
            return Err("attack_ms out of range [1, 1000]");
        }
        if !self.decay_ms.is_finite() || self.decay_ms < 10.0 || self.decay_ms > 5000.0 {
            return Err("decay_ms out of range [10, 5000]");
        }
        Ok(())
    }
}

pub const FFT_SIZE: usize = 512;
pub const SAMPLE_RATE: f32 = 22050.0;
pub const BANDS: usize = 16;
const TWO_PI: f32 = std::f32::consts::TAU;

const EDGES: [usize; BANDS + 1] = [
    1, 2, 3, 4, 5, 7, 9, 12, 16, 21, 28, 37, 49, 65, 86, 114, 256,
];

/// Hardware-independent analysis of mono PCM audio blocks.
pub struct MicDsp {
    re: [f32; FFT_SIZE],
    im: [f32; FFT_SIZE],
    smooth: [f32; BANDS],
    level: f32,
    bass_avg: f32,
    prev_bass: f32,
    last_onset_ms: Option<u64>,
    intervals_ms: [u64; 4],
    interval_count: usize,
    interval_next: usize,
    bpm: f32,
    agc_gain_db: f32,
    beats: u32,
    beat_init: bool,
    pub raw_dbfs: f32,
    pub applied_gain_db: f32,
}

impl Default for MicDsp {
    fn default() -> Self {
        Self {
            re: [0.0; FFT_SIZE],
            im: [0.0; FFT_SIZE],
            smooth: [0.0; BANDS],
            level: 0.0,
            bass_avg: 0.0,
            prev_bass: 0.0,
            last_onset_ms: None,
            intervals_ms: [0; 4],
            interval_count: 0,
            interval_next: 0,
            bpm: 0.0,
            agc_gain_db: 30.0,
            beats: 0,
            beat_init: false,
            raw_dbfs: -180.0,
            applied_gain_db: 30.0,
        }
    }
}

impl MicDsp {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn reset(&mut self) {
        *self = Self::default();
    }

    pub fn beat_count(&self) -> u32 {
        self.beats
    }

    /// Process a block of 512 mono samples in range [-1.0, 1.0].
    pub fn process(
        &mut self,
        samples: &[f32],
        settings: &MicSettings,
        now_ms: u64,
    ) -> AudioSnapshot {
        self.process_at_sample_rate(samples, settings, now_ms, SAMPLE_RATE as u32)
    }

    /// Process a mono PCM block using its actual sample rate and end timestamp.
    /// Invalid sample rates fall back to the crate's default rate.
    pub fn process_at_sample_rate(
        &mut self,
        samples: &[f32],
        settings: &MicSettings,
        now_ms: u64,
        sample_rate_hz: u32,
    ) -> AudioSnapshot {
        let n = FFT_SIZE.min(samples.len());
        if n == 0 {
            return AudioSnapshot::default();
        }
        let sample_rate = if sample_rate_hz == 0 {
            SAMPLE_RATE
        } else {
            sample_rate_hz as f32
        };
        let gain_db = finite_or(settings.gain_db, 30.0).clamp(0.0, 60.0);
        let squelch_db = finite_or(settings.squelch_db, -60.0).clamp(-100.0, -10.0);
        let beat_threshold = finite_or(settings.beat_threshold, 1.6).clamp(1.0, 5.0);
        let attack_ms = finite_or(settings.attack_ms, 40.0).clamp(1.0, 1000.0);
        let decay_ms = finite_or(settings.decay_ms, 300.0).clamp(10.0, 5000.0);

        let mut mean = 0.0f32;
        for &s in &samples[..n] {
            mean += finite_sample(s);
        }
        mean /= n as f32;

        let mut sum = 0.0f32;
        for (i, &sample) in samples.iter().take(n).enumerate() {
            let v = finite_sample(sample) - mean;
            sum += v * v;
            // Hann window
            let window = 0.5 - 0.5 * (TWO_PI * (i as f32) / ((FFT_SIZE - 1) as f32)).cos();
            self.re[i] = v * window;
            self.im[i] = 0.0;
        }
        for i in n..FFT_SIZE {
            self.re[i] = 0.0;
            self.im[i] = 0.0;
        }

        self.raw_dbfs = 20.0 * ((sum / (n as f32)).sqrt() + 1e-9).log10();
        if !settings.agc {
            self.agc_gain_db = gain_db;
        }
        self.applied_gain_db = self.agc_gain_db;
        let gated = self.raw_dbfs < squelch_db;

        let mut target = [0.0f32; BANDS];
        if !gated {
            fft(&mut self.re, &mut self.im);
            for b in 0..BANDS {
                let mut peak = 0.0f32;
                let bin_scale = sample_rate / SAMPLE_RATE;
                let lo = ((EDGES[b] as f32 * bin_scale) as usize).clamp(1, FFT_SIZE / 2 - 1);
                let hi = ((EDGES[b + 1] as f32 * bin_scale) as usize).clamp(lo + 1, FFT_SIZE / 2);
                for k in lo..hi {
                    let mag = (self.re[k] * self.re[k] + self.im[k] * self.im[k]).sqrt()
                        * (4.0 / (FFT_SIZE as f32));
                    peak = peak.max(mag);
                }
                // +0.75 dB per band tilts spectrum
                let db = 20.0 * (peak + 1e-9).log10() + self.applied_gain_db + (b as f32) * 0.75;
                target[b] = clamp01((db + 60.0) / 60.0);
            }
        }

        let level_target = if gated {
            0.0
        } else {
            clamp01((self.raw_dbfs + self.applied_gain_db + 60.0) / 60.0)
        };

        if settings.agc && !gated {
            let mut peak = 0.0f32;
            for &v in &target {
                peak = peak.max(v);
            }
            self.agc_gain_db = (self.agc_gain_db + (0.75 - peak) * 0.6).clamp(0.0, 60.0);
        }

        let dt = n as f32 * 1000.0 / sample_rate;
        let up = 1.0 - (-dt / attack_ms).exp();
        let down = 1.0 - (-dt / decay_ms).exp();

        let mut snapshot = AudioSnapshot {
            available: true,
            ..Default::default()
        };

        for ((smooth_val, snap_band), &target_val) in
            self.smooth.iter_mut().zip(&mut snapshot.bands).zip(&target)
        {
            let factor = if target_val > *smooth_val { up } else { down };
            *smooth_val += (target_val - *smooth_val) * factor;
            *snap_band = clamp01(*smooth_val);
        }

        let l_factor = if level_target > self.level { up } else { down };
        self.level += (level_target - self.level) * l_factor;
        snapshot.level = clamp01(self.level);

        let mut peak_val = snapshot.level;
        for &b in &snapshot.bands {
            peak_val = peak_val.max(b);
        }
        snapshot.peak = peak_val;

        // Waveform: 128 signed points in [-1.0, 1.0] from recent samples
        let step = (n / 128).max(1);
        for i in 0..128 {
            let idx = (i * step).min(n.saturating_sub(1));
            snapshot.waveform[i] = finite_sample(samples[idx]);
        }

        let bass_now = (target[0] + target[1] + target[2]) / 3.0;
        if gated {
            self.bass_avg *= (-dt / 250.0).exp();
            self.prev_bass = 0.0;
        } else if !self.beat_init {
            self.bass_avg = bass_now;
            self.beat_init = true;
        }

        let novelty = (bass_now - self.prev_bass).max(0.0);
        let onset_level = clamp01(novelty * 4.0);
        let refractory_ms = 240;
        let onset_detected = !gated
            && bass_now > 0.12
            && bass_now > self.bass_avg * beat_threshold
            && novelty > 0.035
            && self
                .last_onset_ms
                .is_none_or(|last| now_ms.saturating_sub(last) >= refractory_ms);
        if onset_detected {
            self.beats = self.beats.wrapping_add(1);
            if let Some(previous) = self.last_onset_ms {
                let interval = now_ms.saturating_sub(previous);
                if (270..=1200).contains(&interval) {
                    self.intervals_ms[self.interval_next] = interval;
                    self.interval_next = (self.interval_next + 1) % self.intervals_ms.len();
                    self.interval_count = (self.interval_count + 1).min(self.intervals_ms.len());
                    let mut ordered = [0u64; 4];
                    ordered[..self.interval_count]
                        .copy_from_slice(&self.intervals_ms[..self.interval_count]);
                    ordered[..self.interval_count].sort_unstable();
                    let median = ordered[self.interval_count / 2] as f32;
                    let candidate = 60_000.0 / median;
                    if (50.0..=220.0).contains(&candidate) {
                        self.bpm = candidate;
                    }
                } else {
                    self.interval_count = 0;
                    self.interval_next = 0;
                    self.bpm = 0.0;
                }
            }
            self.last_onset_ms = Some(now_ms);
        }
        let avg_factor = 1.0 - (-dt / 500.0).exp();
        self.bass_avg += (bass_now - self.bass_avg) * avg_factor;
        self.prev_bass = bass_now;
        if self
            .last_onset_ms
            .is_none_or(|last| now_ms.saturating_sub(last) > 2500)
        {
            self.bpm = 0.0;
        }
        snapshot.bass = bass_now;
        snapshot.onset = if onset_detected { onset_level } else { 0.0 };
        snapshot.bpm = self.bpm;
        snapshot.beat = onset_detected;

        snapshot
    }
}

#[inline]
fn finite_or(value: f32, fallback: f32) -> f32 {
    if value.is_finite() {
        value
    } else {
        fallback
    }
}

#[inline]
fn finite_sample(value: f32) -> f32 {
    if value.is_finite() {
        value.clamp(-1.0, 1.0)
    } else {
        0.0
    }
}

#[inline]
fn clamp01(v: f32) -> f32 {
    if !v.is_finite() || v <= 0.0 {
        0.0
    } else if v >= 1.0 {
        1.0
    } else {
        v
    }
}

/// In-place radix-2 Cooley-Tukey FFT for N=512
fn fft(re: &mut [f32; FFT_SIZE], im: &mut [f32; FFT_SIZE]) {
    let mut j = 0;
    for i in 1..FFT_SIZE {
        let mut bit = FFT_SIZE >> 1;
        while j & bit != 0 {
            j ^= bit;
            bit >>= 1;
        }
        j ^= bit;
        if i < j {
            re.swap(i, j);
            im.swap(i, j);
        }
    }

    let mut len = 2;
    while len <= FFT_SIZE {
        let angle = -TWO_PI / (len as f32);
        let wr = angle.cos();
        let wi = angle.sin();

        let mut i = 0;
        while i < FFT_SIZE {
            let mut cr = 1.0f32;
            let mut ci = 0.0f32;
            let half = len / 2;
            for k in 0..half {
                let a = i + k;
                let b = a + half;
                let xr = re[b] * cr - im[b] * ci;
                let xi = re[b] * ci + im[b] * cr;
                re[b] = re[a] - xr;
                im[b] = im[a] - xi;
                re[a] += xr;
                im[a] += xi;

                let t = cr * wr - ci * wi;
                ci = cr * wi + ci * wr;
                cr = t;
            }
            i += len;
        }
        len <<= 1;
    }
}

mod techno;
pub use techno::{generate_synthetic_audio, synthetic_audio_sample};

/// Generates a realistic synthetic audio snapshot based on a 5-second techno beat loop (120 BPM).
pub fn synthetic_audio(now_ms: u64) -> AudioSnapshot {
    generate_synthetic_audio(now_ms)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_synthetic_audio_ranges() {
        for t in [0, 50, 100, 250, 499, 500, 1000, 10000] {
            let snap = synthetic_audio(t);
            assert!(snap.available);
            assert!((0.0..=1.0).contains(&snap.level));
            assert!((0.0..=1.0).contains(&snap.peak));
            for b in snap.bands {
                assert!((0.0..=1.0).contains(&b));
            }
            for w in snap.waveform {
                assert!((-1.0..=1.0).contains(&w));
            }
        }
    }

    #[test]
    fn test_stale_mic_fade_250ms_plus_500ms() {
        let make_active_snapshot = || AudioSnapshot {
            bands: [0.6; 16],
            waveform: [0.5; 128],
            level: 0.8,
            peak: 0.9,
            bass: 0.6,
            onset: 0.8,
            bpm: 120.0,
            beat: true,
            available: true,
        };

        // Fresh within 250ms window: retains full values and available == true
        for age in [0, 50, 150, 249, 250] {
            let mut snap = make_active_snapshot();
            snap.apply_staleness(age);
            assert!(snap.available);
            assert!(snap.beat);
            assert!((snap.level - 0.8).abs() < 1e-6);
            assert!((snap.peak - 0.9).abs() < 1e-6);
            assert!((snap.bands[0] - 0.6).abs() < 1e-6);
            assert!((snap.waveform[0] - 0.5).abs() < 1e-6);
        }

        // Just stale (> 250ms): immediately available = false and beat = false
        let mut snap_251 = make_active_snapshot();
        snap_251.apply_staleness(251);
        assert!(!snap_251.available);
        assert!(!snap_251.beat);
        assert_eq!(snap_251.onset, 0.0);
        assert_eq!(snap_251.bpm, 0.0);
        assert!(snap_251.level < 0.8);

        // Midway through 500ms fade (250ms stale threshold + 250ms = 500ms): gain = 0.5
        let mut snap_500 = make_active_snapshot();
        snap_500.apply_staleness(500);
        assert!(!snap_500.available);
        assert!(!snap_500.beat);
        assert!((snap_500.level - 0.4).abs() < 1e-4);
        assert!((snap_500.peak - 0.45).abs() < 1e-4);
        assert!((snap_500.bands[0] - 0.3).abs() < 1e-4);
        assert!((snap_500.waveform[0] - 0.25).abs() < 1e-4);
        assert!((snap_500.bass - 0.3).abs() < 1e-4);
        assert_eq!(snap_500.bpm, 0.0);
        assert_eq!(snap_500.onset, 0.0);

        // Fade complete at 750ms (250ms + 500ms): all faded to silence
        let mut snap_750 = make_active_snapshot();
        snap_750.apply_staleness(750);
        assert!(!snap_750.available);
        assert!(!snap_750.beat);
        assert_eq!(snap_750.level, 0.0);
        assert_eq!(snap_750.peak, 0.0);
        assert_eq!(snap_750.bass, 0.0);
        assert_eq!(snap_750.bpm, 0.0);
        assert_eq!(snap_750.onset, 0.0);
        assert!(snap_750.bands.iter().all(|&b| b == 0.0));
        assert!(snap_750.waveform.iter().all(|&w| w == 0.0));

        // Past fade duration (e.g. 1000ms): completely silent
        let mut snap_1000 = make_active_snapshot();
        snap_1000.apply_staleness(1000);
        assert!(!snap_1000.available);
        assert_eq!(snap_1000.level, 0.0);
        assert!(snap_1000.bands.iter().all(|&b| b == 0.0));
        assert!(snap_1000.waveform.iter().all(|&w| w == 0.0));
    }

    #[test]
    fn test_waveform_signed_vs_unipolar_convention() {
        let mut dsp = MicDsp::new();
        let settings = MicSettings::default();

        // 1. Silence: waveform is centered at 0.0 (signed silence, NOT 0.5), bands/level 0.0
        let silent = [0.0f32; FFT_SIZE];
        let snap_silent = dsp.process(&silent, &settings, 0);
        for &w in &snap_silent.waveform {
            assert_eq!(w, 0.0);
        }
        for &b in &snap_silent.bands {
            assert_eq!(b, 0.0);
        }

        // 2. Negative PCM samples: waveform preserves negative sign in [-1.0, 0.0),
        // but bands, level, peak are unipolar energy in [0.0, 1.0].
        let mut negative_pcm = [0.0f32; FFT_SIZE];
        for (i, sample) in negative_pcm.iter_mut().enumerate() {
            // Negative half-wave
            let s = (TWO_PI * 440.0 * (i as f32) / SAMPLE_RATE).sin();
            *sample = -s.abs();
        }
        let snap_neg = dsp.process(&negative_pcm, &settings, 100);
        assert!(snap_neg.waveform.iter().any(|&w| w < -0.1));
        // All bands and energy metrics must remain strictly non-negative (unipolar)
        for &b in &snap_neg.bands {
            assert!((0.0..=1.0).contains(&b));
        }
        assert!((0.0..=1.0).contains(&snap_neg.level));
        assert!((0.0..=1.0).contains(&snap_neg.peak));

        // 3. Positive PCM samples: waveform has positive values
        let mut positive_pcm = [0.0f32; FFT_SIZE];
        for (i, sample) in positive_pcm.iter_mut().enumerate() {
            let s = (TWO_PI * 440.0 * (i as f32) / SAMPLE_RATE).sin();
            *sample = s.abs();
        }
        let snap_pos = dsp.process(&positive_pcm, &settings, 200);
        assert!(snap_pos.waveform.iter().any(|&w| w > 0.1));

        // 4. Synthetic audio waveform spans both positive and negative values while bands are unipolar
        let snap_syn = synthetic_audio(300);
        assert!(snap_syn.waveform.iter().any(|&w| w < -0.05));
        assert!(snap_syn.waveform.iter().any(|&w| w > 0.05));
        for &b in &snap_syn.bands {
            assert!((0.0..=1.0).contains(&b));
        }
    }

    #[test]
    fn test_mic_dsp_silence_and_sine() {
        let mut dsp = MicDsp::new();
        let settings = MicSettings::default();

        // Feed silence
        let silent = [0.0f32; FFT_SIZE];
        let snap = dsp.process(&silent, &settings, 0);
        assert!(snap.available);
        assert!(snap.level <= 0.05);

        // Feed 440 Hz sine wave
        let mut sine = [0.0f32; FFT_SIZE];
        for (i, sample) in sine.iter_mut().enumerate() {
            *sample = (TWO_PI * 440.0 * (i as f32) / SAMPLE_RATE).sin();
        }
        let snap_sine = dsp.process(&sine, &settings, 100);
        assert!(snap_sine.level > 0.0);
    }

    fn process_kick_train(bpm: f32, sample_rate: u32, duration_ms: u64) -> (MicDsp, AudioSnapshot) {
        let mut dsp = MicDsp::new();
        let settings = MicSettings {
            agc: false,
            gain_db: 30.0,
            ..MicSettings::default()
        };
        let total_samples = (duration_ms as u128 * sample_rate as u128 / 1000) as usize;
        let period_sec = 60.0 / bpm;
        let mut last_snapshot = AudioSnapshot::default();

        for block_start in (0..total_samples).step_by(FFT_SIZE) {
            let block_end = (block_start + FFT_SIZE).min(total_samples);
            let samples: Vec<f32> = (block_start..block_end)
                .map(|sample_idx| {
                    let t = sample_idx as f32 / sample_rate as f32;
                    let phase = t % period_sec;
                    if phase < 0.12 {
                        0.9 * (-phase / 0.055).exp() * (TWO_PI * 70.0 * phase).sin()
                    } else {
                        0.0
                    }
                })
                .collect();
            let now_ms = (block_end as u128 * 1000 / sample_rate as u128) as u64;
            last_snapshot = dsp.process_at_sample_rate(&samples, &settings, now_ms, sample_rate);
        }
        (dsp, last_snapshot)
    }

    #[test]
    fn deterministic_pcm_kicks_estimate_80_120_and_160_bpm() {
        for expected_bpm in [80.0, 120.0, 160.0] {
            let (dsp, snapshot) = process_kick_train(expected_bpm, 22_050, 8_000);
            assert!(
                (snapshot.bpm - expected_bpm).abs() < 6.0,
                "expected {expected_bpm} BPM, detected {}",
                snapshot.bpm
            );
            assert!(
                dsp.beat_count() >= 7,
                "too few onsets at {expected_bpm} BPM"
            );
            assert!(snapshot.bass.is_finite() && snapshot.onset.is_finite());
        }
    }

    #[test]
    fn deterministic_pcm_bpm_is_sample_rate_and_block_time_correct() {
        let (_, at_22050) = process_kick_train(120.0, 22_050, 6_000);
        let (_, at_44100) = process_kick_train(120.0, 44_100, 6_000);
        assert!(
            (at_22050.bpm - 120.0).abs() < 6.0,
            "22.05kHz: {}",
            at_22050.bpm
        );
        assert!(
            (at_44100.bpm - 120.0).abs() < 6.0,
            "44.1kHz: {}",
            at_44100.bpm
        );
        assert!((at_22050.bpm - at_44100.bpm).abs() < 4.0);
    }

    #[test]
    fn deterministic_pcm_tempo_change_is_followed_and_silence_expires_bpm() {
        let sample_rate = 22_050u32;
        let mut dsp = MicDsp::new();
        let settings = MicSettings {
            agc: false,
            ..MicSettings::default()
        };
        let mut last = AudioSnapshot::default();
        let mut sample_idx = 0usize;
        let total_samples = 9 * sample_rate as usize;
        while sample_idx < total_samples {
            let block_end = (sample_idx + FFT_SIZE).min(total_samples);
            let bpm = if sample_idx as f32 / (sample_rate as f32) < 4.5 {
                80.0
            } else {
                160.0
            };
            let period = 60.0 / bpm;
            let samples: Vec<f32> = (sample_idx..block_end)
                .map(|idx| {
                    let t = idx as f32 / sample_rate as f32;
                    let phase = t % period;
                    if phase < 0.12 {
                        0.9 * (-phase / 0.055).exp() * (TWO_PI * 70.0 * phase).sin()
                    } else {
                        0.0
                    }
                })
                .collect();
            let now_ms = (block_end as u128 * 1000 / sample_rate as u128) as u64;
            last = dsp.process_at_sample_rate(&samples, &settings, now_ms, sample_rate);
            sample_idx = block_end;
        }
        assert!(
            (last.bpm - 160.0).abs() < 10.0,
            "tempo change not tracked: {}",
            last.bpm
        );

        let silent = [0.0f32; FFT_SIZE];
        let block_ms = (FFT_SIZE as u64 * 1000).div_ceil(sample_rate as u64);
        for block in 1..=180u64 {
            last = dsp.process_at_sample_rate(
                &silent,
                &settings,
                9_000 + block * block_ms,
                sample_rate,
            );
        }
        assert_eq!(last.bpm, 0.0, "tempo must expire during silence");
        assert!(!last.beat);
        assert_eq!(last.onset, 0.0);
        assert!(last.bass < 0.05);

        let mut stale = AudioSnapshot { bpm: 120.0, ..last };
        stale.apply_staleness(AUDIO_STALE_AFTER_MS + 1);
        assert_eq!(stale.bpm, 0.0);
    }

    #[test]
    fn non_finite_pcm_never_poison_analysis_state() {
        let mut dsp = MicDsp::new();
        let settings = MicSettings::default();
        let mut samples = [0.0; FFT_SIZE];
        samples[0] = f32::NAN;
        samples[1] = f32::INFINITY;
        samples[2] = f32::NEG_INFINITY;
        samples[3] = f32::MAX;
        let snapshot = dsp.process_at_sample_rate(&samples, &settings, 0, 0);
        assert!(snapshot.level.is_finite());
        assert!(snapshot.peak.is_finite());
        assert!(snapshot.bass.is_finite());
        assert!(snapshot.onset.is_finite());
        assert!(snapshot.bpm.is_finite());
        assert!(snapshot.bands.iter().all(|value| value.is_finite()));
        assert!(snapshot.waveform.iter().all(|value| value.is_finite()));
    }
}
