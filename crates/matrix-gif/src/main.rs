//! `matrix-gif`: renders every built-in LED matrix effect to an animated GIF plus
//! a 2x3 contact-sheet PNG, and records per-effect render statistics.
//!
//! Rendering is fully deterministic: frame `i` is rendered at
//! `t = i * 1000 / fps` milliseconds with a synthetic (or silent) audio snapshot,
//! so the same command reproduces byte-identical GIFs.

mod contact;
mod gallery;
mod gif;
mod model;
mod render;

use std::panic::{catch_unwind, AssertUnwindSafe};
use std::path::{Path, PathBuf};

use clap::{Parser, Subcommand};
use matrix_core::{Engine, Frame};
use serde::Serialize;

use crate::model::{AudioChoice, EffectInfo, EffectSummary};

#[derive(Parser, Debug)]
#[command(
    name = "matrix-gif",
    version,
    about = "Render LED matrix effects to animated GIFs and contact sheets"
)]
struct Cli {
    #[command(subcommand)]
    command: Option<Command>,

    /// Output directory for `<index>-<id>.gif` files.
    #[arg(long, value_name = "DIR", default_value = "docs/effects/gifs")]
    out: PathBuf,

    /// Render every effect in catalog order.
    #[arg(long)]
    all: bool,

    /// Effect id to render (repeatable). Omit together with --all to render all.
    #[arg(long = "effect", value_name = "ID")]
    effects: Vec<String>,

    /// Clip length in seconds.
    #[arg(long, default_value_t = 4.0)]
    seconds: f64,

    /// Frames per second.
    #[arg(long, default_value_t = 25)]
    fps: u32,

    /// Nearest-neighbour upscale factor applied to each 32x32 frame.
    #[arg(long, default_value_t = 5)]
    scale: u32,

    /// Audio feed for audio-reactive effects.
    #[arg(long, value_enum, default_value_t = AudioChoice::Synthetic)]
    audio: AudioChoice,

    /// Write a JSON summary of the render to this path.
    #[arg(long, value_name = "PATH")]
    summary: Option<PathBuf>,
}

#[derive(Subcommand, Debug)]
enum Command {
    /// Render a 2x3 PNG contact sheet per effect instead of GIFs.
    ContactSheets {
        /// Output directory for `<index>-<id>.png` files.
        #[arg(
            long,
            value_name = "DIR",
            default_value = "docs/effects/contact-sheets"
        )]
        out: PathBuf,

        /// Render every effect in catalog order.
        #[arg(long)]
        all: bool,

        /// Effect id to render (repeatable).
        #[arg(long = "effect", value_name = "ID")]
        effects: Vec<String>,

        /// Clip length in seconds.
        #[arg(long, default_value_t = 4.0)]
        seconds: f64,

        /// Frames per second.
        #[arg(long, default_value_t = 25)]
        fps: u32,

        /// Nearest-neighbour upscale factor per cell (each cell is scale*32 px).
        #[arg(long, default_value_t = 4)]
        scale: u32,

        /// Audio feed for audio-reactive effects.
        #[arg(long, value_enum, default_value_t = AudioChoice::Synthetic)]
        audio: AudioChoice,

        /// Write a JSON summary of the render to this path.
        #[arg(long, value_name = "PATH")]
        summary: Option<PathBuf>,
    },
    /// Write the static `index.html` gallery for already-rendered artifacts.
    Gallery {
        /// Directory holding `gifs/` and `contact-sheets/`.
        #[arg(long, value_name = "DIR", default_value = "docs/effects")]
        dir: PathBuf,

        /// Summary produced by the GIF run.
        #[arg(long, value_name = "PATH", default_value = "docs/effects/summary.json")]
        summary: PathBuf,

        /// Output file for the gallery page.
        #[arg(long, value_name = "PATH", default_value = "docs/effects/index.html")]
        out: PathBuf,

        /// Output file for the GitHub-viewable markdown gallery. GitHub does not
        /// render `.html` from a repository, so this is what scrolls on github.com.
        #[arg(
            long,
            value_name = "PATH",
            default_value = "docs/effects/GALLERY.md"
        )]
        markdown: PathBuf,

        /// Repository URL linked in the page header.
        #[arg(
            long,
            default_value = "https://github.com/nicolai455/led-matrix-anim-rs"
        )]
        repo: String,
    },
}

/// Top-level summary document written to `--summary`.
#[derive(Debug, Serialize)]
struct RunSummary {
    generated_by: &'static str,
    fps: u32,
    seconds: f64,
    scale: u32,
    audio: &'static str,
    warmup_frames: u64,
    total_effects: usize,
    rendered: usize,
    failed: usize,
    total_bytes: u64,
    effects: Vec<EffectSummary>,
}

fn main() {
    let cli = Cli::parse();
    let code = match run(cli) {
        Ok(code) => code,
        Err(error) => {
            eprintln!("matrix-gif: {error}");
            1
        }
    };
    if code != 0 {
        std::process::exit(code);
    }
}

fn run(cli: Cli) -> Result<i32, String> {
    match cli.command {
        Some(Command::ContactSheets {
            out,
            all,
            effects,
            seconds,
            fps,
            scale,
            audio,
            summary,
        }) => contact_sheets(
            &out,
            all,
            effects,
            seconds,
            fps,
            scale,
            audio,
            summary.as_deref(),
        ),
        Some(Command::Gallery {
            dir,
            summary,
            out,
            markdown,
            repo,
        }) => gallery_page(&dir, &summary, &out, &markdown, &repo),
        None => gifs(
            &cli.out,
            cli.all,
            cli.effects,
            cli.seconds,
            cli.fps,
            cli.scale,
            cli.audio,
            cli.summary.as_deref(),
        ),
    }
}

fn validate(seconds: f64, fps: u32) -> Result<(), String> {
    if !seconds.is_finite() || seconds <= 0.0 {
        return Err(format!("--seconds must be > 0, got {seconds}"));
    }
    if fps == 0 {
        return Err("--fps must be >= 1".to_string());
    }
    Ok(())
}

/// Resolves the effects to render: `--all` wins, then explicit `--effect`
/// values (in catalog order), otherwise every effect.
fn select(effects: &[EffectInfo], all: bool, wanted: &[String]) -> Result<Vec<EffectInfo>, String> {
    if all || wanted.is_empty() {
        return Ok(effects.to_vec());
    }
    let mut chosen = Vec::with_capacity(wanted.len());
    for id in wanted {
        let found = effects
            .iter()
            .find(|effect| {
                effect.id.eq_ignore_ascii_case(id) || effect.name.eq_ignore_ascii_case(id)
            })
            .ok_or_else(|| format!("unknown effect '{id}'"))?;
        if !chosen
            .iter()
            .any(|existing: &EffectInfo| existing.id == found.id)
        {
            chosen.push(found.clone());
        }
    }
    chosen.sort_by_key(|effect| effect.index);
    Ok(chosen)
}

#[allow(clippy::too_many_arguments)]
fn gifs(
    out: &Path,
    all: bool,
    wanted: Vec<String>,
    seconds: f64,
    fps: u32,
    scale: u32,
    audio: AudioChoice,
    summary_path: Option<&Path>,
) -> Result<i32, String> {
    validate(seconds, fps)?;
    if scale == 0 {
        return Err("--scale must be >= 1".to_string());
    }
    let mut engine = render::new_engine()?;
    let catalog = render::discover_effects(&engine)?;
    let chosen = select(&catalog, all, &wanted)?;
    let frames = render::frame_count(seconds, fps);
    std::fs::create_dir_all(out)
        .map_err(|error| format!("cannot create {}: {error}", out.display()))?;

    let mut rows = Vec::with_capacity(chosen.len());
    for info in &chosen {
        let path = out.join(format!("{}.gif", info.file_stem()));
        match render_gif(&mut engine, info, &path, seconds, fps, scale, audio, frames) {
            Ok(row) => {
                println!(
                    "[{:>3}/{}] {:<28} {:>7} bytes  max_luma={:<3} nonblack={}",
                    info.index + 1,
                    catalog.len(),
                    info.id,
                    row.bytes,
                    row.max_luma,
                    row.nonblack_frames
                );
                rows.push(row);
            }
            Err(error) => {
                println!(
                    "[{:>3}/{}] {:<28} FAILED: {}",
                    info.index + 1,
                    catalog.len(),
                    info.id,
                    error
                );
                rows.push(EffectSummary::for_error(info, fps, seconds, error));
                // A panicking effect can leave its own state half-updated; rebuild
                // the engine so the next effect starts from a clean slate.
                engine = render::new_engine()?;
            }
        }
    }

    finish(
        &rows,
        summary_path,
        fps,
        seconds,
        scale,
        audio,
        catalog.len(),
    )
}

#[allow(clippy::too_many_arguments)]
fn render_gif(
    engine: &mut Engine,
    info: &EffectInfo,
    path: &Path,
    seconds: f64,
    fps: u32,
    scale: u32,
    audio: AudioChoice,
    frames: u64,
) -> Result<EffectSummary, String> {
    render::activate(engine, &info.id, audio)?;
    render::warmup(engine, fps, audio);

    let stats = gif::write_gif(path, scale, fps, frames, |index| {
        let now_ms = render::frame_time_ms(index, fps);
        let snapshot = audio.snapshot(now_ms);
        // A single broken effect must not abort the whole run: catch the panic,
        // report it in the summary and move on to the next effect.
        catch_unwind(AssertUnwindSafe(|| engine.render(now_ms, &snapshot))).map_err(|payload| {
            format!(
                "effect panicked at frame {index} (t={now_ms} ms): {}",
                panic_message(payload)
            )
        })
    })?;

    let bytes = std::fs::metadata(path)
        .map(|meta| meta.len())
        .map_err(|error| format!("cannot stat {}: {error}", path.display()))?;

    Ok(EffectSummary {
        index: info.index,
        id: info.id.clone(),
        name: info.name.clone(),
        frames: stats.frames,
        fps,
        seconds,
        max_luma: stats.max_luma,
        mean_luma: round2(stats.mean_luma()),
        nonblack_frames: stats.nonblack_frames,
        distinct_colors_max: stats.distinct_colors_max,
        has_parameters: info.has_parameters(),
        audio_reactive: info.audio_reactive,
        bytes,
        error: None,
    })
}

#[allow(clippy::too_many_arguments)]
fn contact_sheets(
    out: &Path,
    all: bool,
    wanted: Vec<String>,
    seconds: f64,
    fps: u32,
    scale: u32,
    audio: AudioChoice,
    summary_path: Option<&Path>,
) -> Result<i32, String> {
    validate(seconds, fps)?;
    if scale == 0 {
        return Err("--scale must be >= 1".to_string());
    }
    let mut engine = render::new_engine()?;
    let catalog = render::discover_effects(&engine)?;
    let chosen = select(&catalog, all, &wanted)?;
    let frames = render::frame_count(seconds, fps);
    std::fs::create_dir_all(out)
        .map_err(|error| format!("cannot create {}: {error}", out.display()))?;

    let mut rows = Vec::with_capacity(chosen.len());
    for info in &chosen {
        let path = out.join(format!("{}.png", info.file_stem()));
        match render_sheet(&mut engine, info, &path, seconds, fps, scale, audio, frames) {
            Ok(row) => {
                println!(
                    "[{:>3}/{}] {:<28} {:>7} bytes  contact sheet",
                    info.index + 1,
                    catalog.len(),
                    info.id,
                    row.bytes
                );
                rows.push(row);
            }
            Err(error) => {
                println!(
                    "[{:>3}/{}] {:<28} FAILED: {}",
                    info.index + 1,
                    catalog.len(),
                    info.id,
                    error
                );
                rows.push(EffectSummary::for_error(info, fps, seconds, error));
            }
        }
    }

    finish(
        &rows,
        summary_path,
        fps,
        seconds,
        scale,
        audio,
        catalog.len(),
    )
}

#[allow(clippy::too_many_arguments)]
fn render_sheet(
    engine: &mut Engine,
    info: &EffectInfo,
    path: &Path,
    seconds: f64,
    fps: u32,
    scale: u32,
    audio: AudioChoice,
    frames: u64,
) -> Result<EffectSummary, String> {
    render::activate(engine, &info.id, audio)?;
    render::warmup(engine, fps, audio);

    let wanted: Vec<u64> = contact::sample_indices(frames);
    let mut samples: Vec<Option<Frame>> = vec![None; wanted.len()];
    let mut stats = gif::GifStats::default();
    let mut next = 0usize;

    for index in 0..frames {
        let now_ms = render::frame_time_ms(index, fps);
        let snapshot = audio.snapshot(now_ms);
        let frame = catch_unwind(AssertUnwindSafe(|| engine.render(now_ms, &snapshot))).map_err(
            |payload| {
                format!(
                    "effect panicked at frame {index} (t={now_ms} ms): {}",
                    panic_message(payload)
                )
            },
        )?;
        stats.absorb(&frame);
        while next < wanted.len() && wanted[next] == index {
            samples[next] = Some(frame.clone());
            next += 1;
        }
    }

    // Fill any slot the clip was too short for with the last rendered frame so
    // every sheet is complete.
    let last = samples.iter().rev().flatten().next().cloned();
    if last.is_some() {
        for slot in samples.iter_mut() {
            if slot.is_none() {
                slot.clone_from(&last);
            }
        }
    }
    let samples: Vec<Frame> = samples.into_iter().flatten().collect();

    let sheet = contact::build_sheet(info, &samples, scale);
    let bytes = contact::save_png(&sheet, path)?;

    Ok(EffectSummary {
        index: info.index,
        id: info.id.clone(),
        name: info.name.clone(),
        frames,
        fps,
        seconds,
        max_luma: stats.max_luma,
        mean_luma: round2(stats.mean_luma()),
        nonblack_frames: stats.nonblack_frames,
        distinct_colors_max: stats.distinct_colors_max,
        has_parameters: info.has_parameters(),
        audio_reactive: info.audio_reactive,
        bytes,
        error: None,
    })
}

/// Best-effort text of a caught panic payload.
fn panic_message(payload: Box<dyn std::any::Any + Send>) -> String {
    if let Some(message) = payload.downcast_ref::<&str>() {
        (*message).to_string()
    } else if let Some(message) = payload.downcast_ref::<String>() {
        message.clone()
    } else {
        "<non-string panic payload>".to_string()
    }
}

/// Builds `index.html` from an existing summary and verifies that every artifact
/// the page links to is actually present, so a fresh clone never ships a gallery
/// full of 404s.
fn gallery_page(
    dir: &Path,
    summary_path: &Path,
    out: &Path,
    markdown: &Path,
    repo: &str,
) -> Result<i32, String> {
    let raw = std::fs::read_to_string(summary_path)
        .map_err(|error| format!("cannot read {}: {error}", summary_path.display()))?;
    #[derive(serde::Deserialize)]
    struct SummaryDoc {
        fps: u32,
        seconds: f64,
        effects: Vec<EffectSummary>,
    }
    let doc: SummaryDoc = serde_json::from_str(&raw)
        .map_err(|error| format!("cannot parse {}: {error}", summary_path.display()))?;
    if doc.effects.is_empty() {
        return Err(format!("{} contains no effects", summary_path.display()));
    }

    let mut missing = Vec::new();
    for row in &doc.effects {
        let stem = format!(
            "{:03}-{}",
            row.index + 1,
            row.id
                .chars()
                .map(
                    |c| if c.is_ascii_alphanumeric() || c == '-' || c == '_' || c == '.' {
                        c
                    } else {
                        '_'
                    }
                )
                .collect::<String>()
        );
        for candidate in [
            dir.join("gifs").join(format!("{stem}.gif")),
            dir.join("contact-sheets").join(format!("{stem}.png")),
        ] {
            if !candidate.exists() {
                missing.push(candidate.display().to_string());
            }
        }
    }
    if !missing.is_empty() {
        return Err(format!(
            "{} artifacts referenced by the summary are missing (first: {}); render them first",
            missing.len(),
            missing[0]
        ));
    }

    let html = gallery::render(
        &doc.effects,
        &gallery::GalleryMeta {
            seconds: doc.seconds,
            fps: doc.fps,
            repo,
        },
    );
    if let Some(parent) = out.parent() {
        std::fs::create_dir_all(parent)
            .map_err(|error| format!("cannot create {}: {error}", parent.display()))?;
    }
    std::fs::write(out, html)
        .map_err(|error| format!("cannot write {}: {error}", out.display()))?;
    let meta = gallery::GalleryMeta {
        seconds: doc.seconds,
        fps: doc.fps,
        repo,
    };
    let md = gallery::render_markdown(&doc.effects, &meta);
    if let Some(parent) = markdown.parent() {
        std::fs::create_dir_all(parent)
            .map_err(|error| format!("cannot create {}: {error}", parent.display()))?;
    }
    std::fs::write(markdown, md)
        .map_err(|error| format!("cannot write {}: {error}", markdown.display()))?;
    println!(
        "gallery: {} effects -> {} ({} bytes) + {} ({} bytes)",
        doc.effects.len(),
        out.display(),
        std::fs::metadata(out).map(|meta| meta.len()).unwrap_or(0),
        markdown.display(),
        std::fs::metadata(markdown).map(|meta| meta.len()).unwrap_or(0)
    );
    Ok(0)
}

fn finish(
    rows: &[EffectSummary],
    summary_path: Option<&Path>,
    fps: u32,
    seconds: f64,
    scale: u32,
    audio: AudioChoice,
    catalog_len: usize,
) -> Result<i32, String> {
    let failed = rows.iter().filter(|row| row.error.is_some()).count();
    let total_bytes = rows.iter().map(|row| row.bytes).sum();
    if let Some(path) = summary_path {
        let summary = RunSummary {
            generated_by: "matrix-gif",
            fps,
            seconds,
            scale,
            audio: audio.as_str(),
            warmup_frames: render::WARMUP_FRAMES,
            total_effects: catalog_len,
            rendered: rows.len() - failed,
            failed,
            total_bytes,
            effects: rows.to_vec(),
        };
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)
                .map_err(|error| format!("cannot create {}: {error}", parent.display()))?;
        }
        let json = serde_json::to_string_pretty(&summary)
            .map_err(|error| format!("cannot serialize summary: {error}"))?;
        std::fs::write(path, json)
            .map_err(|error| format!("cannot write {}: {error}", path.display()))?;
        println!("summary written to {}", path.display());
    }
    println!(
        "done: {} rendered, {} failed, {} bytes total",
        rows.len() - failed,
        failed,
        total_bytes
    );
    Ok(if failed == 0 { 0 } else { 1 })
}

fn round2(value: f64) -> f64 {
    (value * 100.0).round() / 100.0
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn validate_rejects_bad_input() {
        assert!(validate(0.0, 25).is_err());
        assert!(validate(-1.0, 25).is_err());
        assert!(validate(f64::NAN, 25).is_err());
        assert!(validate(1.0, 0).is_err());
        assert!(validate(4.0, 25).is_ok());
    }

    #[test]
    fn select_defaults_to_everything() {
        let effects = vec![
            EffectInfo {
                index: 0,
                id: "A".into(),
                name: "Alpha".into(),
                parameter_count: 0,
                audio_reactive: false,
            },
            EffectInfo {
                index: 1,
                id: "B".into(),
                name: "Beta".into(),
                parameter_count: 1,
                audio_reactive: true,
            },
        ];
        assert_eq!(select(&effects, false, &[]).unwrap().len(), 2);
        assert_eq!(select(&effects, true, &["A".into()]).unwrap().len(), 2);
        let picked = select(&effects, false, &["beta".into(), "A".into()]).unwrap();
        assert_eq!(picked[0].id, "A");
        assert_eq!(picked[1].id, "B");
        assert!(select(&effects, false, &["nope".into()]).is_err());
    }

    #[test]
    fn select_dedupes_repeated_ids() {
        let effects = vec![EffectInfo {
            index: 0,
            id: "A".into(),
            name: "Alpha".into(),
            parameter_count: 0,
            audio_reactive: false,
        }];
        let picked = select(&effects, false, &["A".into(), "A".into()]).unwrap();
        assert_eq!(picked.len(), 1);
    }

    #[test]
    fn round2_rounds_to_two_places() {
        assert!((round2(1.2345) - 1.23).abs() < f64::EPSILON);
        assert!((round2(1.235) - 1.24).abs() < f64::EPSILON);
    }

    #[test]
    fn panic_message_reads_str_and_string_payloads() {
        let from_str = catch_unwind(|| panic!("boom &str")).unwrap_err();
        assert_eq!(panic_message(from_str), "boom &str");
        let from_string =
            catch_unwind(|| std::panic::panic_any(String::from("boom String"))).unwrap_err();
        assert_eq!(panic_message(from_string), "boom String");
    }

    #[test]
    fn render_panic_is_reported_as_error_text() {
        // Mirrors the per-frame closure used while recording: a panicking render
        // must surface as an Err string (and therefore as a summary error row)
        // instead of unwinding through the whole run.
        let mut calls = 0u32;
        let result: Result<u32, String> = catch_unwind(AssertUnwindSafe(|| {
            calls += 1;
            panic!("simulated effect panic");
        }))
        .map_err(|payload| format!("effect panicked: {}", panic_message(payload)));
        assert_eq!(calls, 1);
        assert_eq!(
            result.unwrap_err(),
            "effect panicked: simulated effect panic"
        );
    }
}
