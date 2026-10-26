use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq)]
pub struct AudioSnapshot {
    pub bands: [f32; 16],
    /// Audio waveform samples in signed range [-1.0, 1.0] (0.0 = silence).
    pub waveform: [f32; 128],
    pub level: f32,
    pub peak: f32,
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
        if age_ms >= AUDIO_STALE_AFTER_MS + AUDIO_FADE_DURATION_MS {
            self.level = 0.0;
            self.peak = 0.0;
            self.beat = false;
            self.bands.fill(0.0);
            self.waveform.fill(0.0);
        } else {
            let fade_age = (age_ms - AUDIO_STALE_AFTER_MS) as f32;
            let gain = (1.0 - fade_age / (AUDIO_FADE_DURATION_MS as f32)).clamp(0.0, 1.0);
            self.level *= gain;
            self.peak *= gain;
            self.beat = false;
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
    agc_gain_db: f32,
    beats: u32,
    last_beat_ms: u64,
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
            agc_gain_db: 30.0,
            beats: 0,
            last_beat_ms: 0,
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
        let n = FFT_SIZE.min(samples.len());
        if n == 0 {
            return AudioSnapshot::default();
        }

        let mut mean = 0.0f32;
        for &s in &samples[..n] {
            mean += s;
        }
        mean /= n as f32;

        let mut sum = 0.0f32;
        for (i, &sample) in samples.iter().take(n).enumerate() {
            let v = sample - mean;
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
            self.agc_gain_db = settings.gain_db;
        }
        self.applied_gain_db = self.agc_gain_db;
        let gated = self.raw_dbfs < settings.squelch_db;

        let mut target = [0.0f32; BANDS];
        if !gated {
            fft(&mut self.re, &mut self.im);
            for b in 0..BANDS {
                let mut peak = 0.0f32;
                for k in EDGES[b]..EDGES[b + 1] {
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

        let dt = (FFT_SIZE as f32) * 1000.0 / SAMPLE_RATE;
        let up = 1.0 - (-dt / settings.attack_ms.max(1.0)).exp();
        let down = 1.0 - (-dt / settings.decay_ms.max(1.0)).exp();

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
            let s = samples[idx];
            snapshot.waveform[i] = if s.is_finite() {
                s.clamp(-1.0, 1.0)
            } else {
                0.0
            };
        }

        let bass_now = (target[0] + target[1] + target[2]) / 3.0;
        if !self.beat_init {
            self.bass_avg = bass_now;
            self.beat_init = true;
        }

        let mut beat_detected = false;
        if bass_now > self.bass_avg * settings.beat_threshold
            && bass_now > 0.2
            && (now_ms.saturating_sub(self.last_beat_ms) > 150)
        {
            self.beats = self.beats.wrapping_add(1);
            self.last_beat_ms = now_ms;
            beat_detected = true;
        }
        self.bass_avg = self.bass_avg * 0.97 + bass_now * 0.03;
        snapshot.beat = beat_detected;

        snapshot
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

        // Fade complete at 750ms (250ms + 500ms): all faded to silence
        let mut snap_750 = make_active_snapshot();
        snap_750.apply_staleness(750);
        assert!(!snap_750.available);
        assert!(!snap_750.beat);
        assert_eq!(snap_750.level, 0.0);
        assert_eq!(snap_750.peak, 0.0);
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
}
