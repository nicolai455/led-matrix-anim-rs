use super::{clamp01, AudioSnapshot, SAMPLE_RATE, TWO_PI};

/// Duration of the repeating techno beat pattern in seconds (10 beats at 120 BPM = 5.0 seconds).
pub const TECHNO_LOOP_DURATION_SEC: f32 = 5.0;
/// Beat interval in seconds at 120 BPM.
pub const TECHNO_BEAT_INTERVAL_SEC: f32 = 0.5;
/// Sixteenth-note step interval in seconds (4 steps per beat).
pub const TECHNO_STEP_INTERVAL_SEC: f32 = 0.125;

/// Generates a single PCM audio sample in [-1.0, 1.0] for the synthetic 5-second techno loop.
///
/// Features:
/// - 120 BPM techno groove (5.0s loop = 10 beats = 40 sixteenth-note steps)
/// - Punchy 4-on-the-floor kick with exponential pitch drop (160 Hz -> 46 Hz) and transient click
/// - Rolling 16th-note sub bassline in D minor with sidechain ducking on kick
/// - 16th-note closed hi-hats with metallic resonance and velocity accents
/// - Upbeat open hi-hat on the offbeat (step 2) with sizzle decay
/// - Snappy clap with multi-flam attack on beats 1, 3, 5, 7
/// - Resonant acid synth stabs with filter envelope sweep in bar 2 (beats 4..7)
/// - Turnaround tom/percussion roll in beats 8 & 9 before looping
pub fn synthetic_audio_sample(time_sec: f32) -> f32 {
    let t_raw = if time_sec.is_finite() && time_sec >= 0.0 {
        time_sec
    } else {
        0.0
    };

    let t_loop = t_raw % TECHNO_LOOP_DURATION_SEC;
    let beat_idx = (t_loop / TECHNO_BEAT_INTERVAL_SEC) as usize; // 0..9
    let t_beat = t_loop - (beat_idx as f32) * TECHNO_BEAT_INTERVAL_SEC;

    let step_in_beat = (t_beat / TECHNO_STEP_INTERVAL_SEC).min(3.0) as usize; // 0..3
    let step_idx = (beat_idx * 4 + step_in_beat).min(39); // 0..39
    let t_step = t_beat - (step_in_beat as f32) * TECHNO_STEP_INTERVAL_SEC;

    // 1. Kick Drum (Beats 0..8)
    let kick = if beat_idx <= 8 {
        let f_start = 160.0f32;
        let f_end = 46.0f32;
        let decay_p = 0.035f32;
        let phi_k = TWO_PI
            * (f_end * t_beat + (f_start - f_end) * decay_p * (1.0 - (-t_beat / decay_p).exp()));
        let env_k = (-t_beat / 0.14).exp();
        let click = 0.35 * (-t_beat / 0.006).exp() * (TWO_PI * 2500.0 * t_beat).sin();
        (phi_k.sin() * env_k + click) * 0.85
    } else {
        0.0
    };

    // 2. Rolling 16th-note Bassline (D minor)
    let duck = (t_beat / 0.09).clamp(0.0, 1.0);
    let freq_b = if beat_idx < 4 {
        73.42 // D2
    } else if beat_idx < 6 {
        if step_in_beat == 1 || step_in_beat == 3 {
            87.31 // F2
        } else {
            73.42 // D2
        }
    } else if beat_idx < 8 {
        if step_in_beat == 2 {
            98.00 // G2
        } else {
            73.42 // D2
        }
    } else {
        65.41 // C2
    };

    let vel_b = match step_in_beat {
        0 => 0.25,
        1 => 0.75,
        2 => 0.95,
        _ => 0.65,
    };
    let env_b = vel_b * (-t_step / 0.085).exp() * duck;
    let phi_b = TWO_PI * freq_b * t_raw;
    let saw_b = phi_b.sin() + 0.45 * (2.0 * phi_b).sin() + 0.25 * (3.0 * phi_b).sin();
    let bass = saw_b * env_b * 0.50;

    // Deterministic pseudo-random noise generator
    let sample_idx = (t_raw * 22050.0) as u32;
    let h = sample_idx.wrapping_mul(1664525).wrapping_add(1013904223);
    let noise = ((h as f32) / 2147483648.0) - 1.0;

    // 3. Closed Hi-Hat (16th notes)
    let metal_ch =
        0.3 * (TWO_PI * 5800.0 * t_raw).sin() + 0.3 * (TWO_PI * 7400.0 * t_raw).sin() + 0.4 * noise;
    let vel_ch = match step_in_beat {
        0 => 0.35,
        1 => 0.60,
        2 => 0.85,
        _ => 0.55,
    };
    let env_ch = vel_ch * (-t_step / 0.028).exp();
    let closed_hat = metal_ch * env_ch * 0.22;

    // 4. Open Hi-Hat (on upbeat: step 2, active from beat 2)
    let open_hat = if beat_idx >= 2 && t_beat >= 0.25 {
        let t_oh = t_beat - 0.25;
        let env_oh = (-t_oh / 0.12).exp();
        let metal_oh = 0.25 * (TWO_PI * 4400.0 * t_raw).sin()
            + 0.35 * (TWO_PI * 6800.0 * t_raw).sin()
            + 0.4 * noise;
        metal_oh * env_oh * 0.30
    } else {
        0.0
    };

    // 5. Snappy Clap (beats 1, 3, 5, 7)
    let clap = if matches!(beat_idx, 1 | 3 | 5 | 7) {
        let mut flam = (-t_beat / 0.008).exp();
        if t_beat >= 0.010 {
            flam += 0.7 * (-(t_beat - 0.010) / 0.008).exp();
        }
        if t_beat >= 0.020 {
            flam += 0.6 * (-(t_beat - 0.020) / 0.008).exp();
        }
        let env_clap_body = (-t_beat / 0.16).exp();
        let clap_tone = (TWO_PI * 240.0 * t_beat).sin();
        (0.2 * clap_tone + 0.8 * noise) * (flam * 0.3 + env_clap_body * 0.7) * 0.42
    } else {
        0.0
    };

    // 6. Acid Synth Stabs (beats 4..7)
    let synth = if (4..=7).contains(&beat_idx) {
        const STABS: [usize; 4] = [18, 22, 26, 30];
        let mut last_stab = None;
        for &s in &STABS {
            if s <= step_idx {
                last_stab = Some(s);
            }
        }
        if let Some(s) = last_stab {
            let t_stab = (step_idx - s) as f32 * TECHNO_STEP_INTERVAL_SEC + t_step;
            if t_stab < 0.35 {
                let cutoff = 800.0 + 1200.0 * (-t_stab / 0.15).exp();
                let f_synth = if s % 8 == 6 { 220.0 } else { 146.83 };
                let phi_s = TWO_PI * f_synth * t_raw;
                let saw_s =
                    0.5 * phi_s.sin() + 0.3 * (2.0 * phi_s).sin() + 0.2 * (3.0 * phi_s).sin();
                let res = (TWO_PI * cutoff * t_stab).sin() * (-t_stab / 0.10).exp();
                (0.7 * saw_s + 0.3 * res) * (-t_stab / 0.18).exp() * 0.35
            } else {
                0.0
            }
        } else {
            0.0
        }
    } else {
        0.0
    };

    // 7. Turnaround Percussion Fill (beats 8 & 9)
    let fill = if beat_idx >= 8 {
        let tom_step = step_idx - 32;
        let tom_f = 240.0 - (tom_step as f32) * 10.0;
        let env_tom = (-t_step / 0.08).exp();
        (0.6 * (TWO_PI * tom_f * t_step).sin() + 0.4 * noise) * env_tom * 0.40
    } else {
        0.0
    };

    let mix = kick + bass + closed_hat + open_hat + clap + synth + fill;
    mix.clamp(-1.0, 1.0)
}

/// Generates a realistic synthetic audio snapshot based on the 5-second techno beat loop.
pub fn generate_synthetic_audio(now_ms: u64) -> AudioSnapshot {
    let now_sec = (now_ms as f32) / 1000.0;
    let t_loop = now_sec % TECHNO_LOOP_DURATION_SEC;

    let beat_idx = (t_loop / TECHNO_BEAT_INTERVAL_SEC) as usize; // 0..9
    let t_beat = t_loop - (beat_idx as f32) * TECHNO_BEAT_INTERVAL_SEC;

    let step_in_beat = (t_beat / TECHNO_STEP_INTERVAL_SEC).min(3.0) as usize; // 0..3
    let step_idx = (beat_idx * 4 + step_in_beat).min(39); // 0..39
    let t_step = t_beat - (step_in_beat as f32) * TECHNO_STEP_INTERVAL_SEC;

    // Component envelopes:
    let env_kick = if beat_idx <= 8 {
        (-t_beat / 0.14).exp()
    } else {
        0.0
    };

    let duck = (t_beat / 0.09).clamp(0.0, 1.0);
    let vel_b = match step_in_beat {
        0 => 0.25,
        1 => 0.75,
        2 => 0.95,
        _ => 0.65,
    };
    let env_bass = vel_b * (-t_step / 0.085).exp() * duck;

    let vel_ch = match step_in_beat {
        0 => 0.35,
        1 => 0.60,
        2 => 0.85,
        _ => 0.55,
    };
    let env_ch = vel_ch * (-t_step / 0.028).exp();

    let env_oh = if beat_idx >= 2 && t_beat >= 0.25 {
        (-(t_beat - 0.25) / 0.12).exp()
    } else {
        0.0
    };

    let env_clap = if matches!(beat_idx, 1 | 3 | 5 | 7) {
        (-t_beat / 0.16).exp()
    } else {
        0.0
    };

    let env_synth = if (4..=7).contains(&beat_idx) {
        const STABS: [usize; 4] = [18, 22, 26, 30];
        let mut last_stab = None;
        for &s in &STABS {
            if s <= step_idx {
                last_stab = Some(s);
            }
        }
        if let Some(s) = last_stab {
            let t_stab = (step_idx - s) as f32 * TECHNO_STEP_INTERVAL_SEC + t_step;
            if t_stab < 0.35 {
                (-t_stab / 0.18).exp()
            } else {
                0.0
            }
        } else {
            0.0
        }
    } else {
        0.0
    };

    let env_fill = if beat_idx >= 8 {
        (-t_step / 0.08).exp()
    } else {
        0.0
    };

    // 16 continuous frequency bands mapping instrument acoustics
    let mut bands = [0.0f32; 16];

    // Low end (Sub, Kick, Bass: Bands 0..4)
    bands[0] = 0.92 * env_kick + 0.35 * env_bass;
    bands[1] = 0.98 * env_kick + 0.65 * env_bass;
    bands[2] = 0.70 * env_kick + 0.85 * env_bass;
    bands[3] = 0.35 * env_kick + 0.72 * env_bass + 0.60 * env_fill;
    bands[4] = 0.15 * env_kick + 0.45 * env_bass + 0.25 * env_clap + 0.50 * env_fill;

    // Mid range (Snare, Clap, Acid Synth, Toms: Bands 5..9)
    bands[5] = 0.20 * env_bass + 0.40 * env_clap + 0.30 * env_synth + 0.45 * env_fill;
    bands[6] = 0.60 * env_clap + 0.55 * env_synth;
    bands[7] = 0.75 * env_clap + 0.70 * env_synth;
    bands[8] = 0.85 * env_clap + 0.75 * env_synth;
    bands[9] = 0.70 * env_clap + 0.80 * env_synth;

    // Highs & Air (Hi-Hats, Sparkle: Bands 10..15)
    bands[10] = 0.45 * env_clap + 0.65 * env_synth + 0.20 * env_oh;
    bands[11] = 0.25 * env_clap + 0.45 * env_synth + 0.35 * env_oh;
    bands[12] = 0.25 * env_ch + 0.55 * env_oh;
    bands[13] = 0.45 * env_ch + 0.80 * env_oh;
    bands[14] = 0.65 * env_ch + 0.92 * env_oh;
    bands[15] = 0.70 * env_ch + 0.80 * env_oh;

    // Ambient floor ensures visualizers always breathe naturally
    let ambient = 0.03 + 0.015 * (now_sec * 4.0).sin();
    for band in &mut bands {
        *band = clamp01(*band + ambient);
    }

    // Single-transient beat detection (active only during the first 28ms of kick attack)
    let beat = (t_beat < 0.028) && (beat_idx <= 8);

    // Multi-band level and peak
    let bass_group = (bands[0] + bands[1] + bands[2] + bands[3]) * 0.25;
    let mid_group = (bands[4] + bands[5] + bands[6] + bands[7] + bands[8] + bands[9]) / 6.0;
    let treble_group =
        (bands[10] + bands[11] + bands[12] + bands[13] + bands[14] + bands[15]) / 6.0;
    let level = clamp01(0.40 * bass_group + 0.35 * mid_group + 0.25 * treble_group);
    let peak = clamp01(bands.iter().copied().fold(level, f32::max));

    // Realistic audio waveform: 128 PCM points sampled over the recent 23.2ms window
    let window_sec = 512.0 / SAMPLE_RATE;
    let dt_w = window_sec / 128.0;
    let mut waveform = [0.0f32; 128];
    for (i, w) in waveform.iter_mut().enumerate() {
        let tw = now_sec - window_sec + (i as f32) * dt_w;
        *w = synthetic_audio_sample(tw);
    }

    AudioSnapshot {
        bands,
        waveform,
        level,
        peak,
        beat,
        available: true,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_techno_sample_bounds_and_finite() {
        for i in 0..1000 {
            let t = (i as f32) * 0.005;
            let sample = synthetic_audio_sample(t);
            assert!(sample.is_finite(), "sample at t={} must be finite", t);
            assert!(
                (-1.0..=1.0).contains(&sample),
                "sample at t={} out of bounds: {}",
                t,
                sample
            );
        }
    }

    #[test]
    fn test_techno_beat_cadence_and_kicks() {
        // At 120 BPM, beats 0..8 occur at 0ms, 500ms, 1000ms, 1500ms, 2000ms, 2500ms, 3000ms, 3500ms, 4000ms
        let mut kick_beats_detected = 0;
        for beat_num in 0..=8 {
            let snap = generate_synthetic_audio(beat_num * 500);
            assert!(
                snap.beat,
                "beat {} at {}ms must be detected",
                beat_num,
                beat_num * 500
            );
            // Low frequencies must dominate on kick
            assert!(
                snap.bands[0] > 0.8 && snap.bands[1] > 0.8,
                "kick drum bass bands must be punchy at beat {}",
                beat_num
            );
            kick_beats_detected += 1;
        }
        assert_eq!(kick_beats_detected, 9);

        // Turnaround at beat 9 (4500ms) is percussion/fill, not a standard kick
        let snap_turnaround = generate_synthetic_audio(4500);
        assert!(
            !snap_turnaround.beat,
            "turnaround beat 9 should not register as downbeat kick"
        );

        // Midway between beats (e.g. 250ms), beat flag must be false
        let snap_offbeat = generate_synthetic_audio(250);
        assert!(
            !snap_offbeat.beat,
            "offbeat at 250ms must not trigger beat flag"
        );
    }

    #[test]
    fn test_techno_frequency_spectrum_diversity() {
        // Unlike the previous flat 3-band stepped synthetic audio, bands must vary continuously across the spectrum
        let snap = generate_synthetic_audio(500); // Kick + Clap
        assert_ne!(snap.bands[0], snap.bands[4]);
        assert_ne!(snap.bands[5], snap.bands[9]);
        assert_ne!(snap.bands[10], snap.bands[15]);

        // Spectrum must span both non-trivial low, mid, and high content
        let low = snap.bands[1];
        let mid = snap.bands[7];
        let high = snap.bands[14];
        assert!(low > 0.5, "lows must be active: {}", low);
        assert!(mid > 0.4, "mids must be active: {}", mid);
        assert!(high > 0.1, "highs must be active: {}", high);
    }
}
