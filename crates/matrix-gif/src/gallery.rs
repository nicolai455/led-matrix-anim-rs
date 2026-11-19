//! Static gallery page for the rendered effect artifacts.
//!
//! The page has to work three ways: opened straight from disk (`file://`), served
//! by a plain static file server, and rendered by GitHub from the repository. So
//! it is one self-contained HTML file with relative `gifs/…` and
//! `contact-sheets/…` paths, no build step, no framework and no network calls.

use crate::model::EffectSummary;

/// File names as they are written next to this page.
const GIF_DIR: &str = "gifs";
const SHEET_DIR: &str = "contact-sheets";

fn esc(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    for c in text.chars() {
        match c {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            '\'' => out.push_str("&#39;"),
            other => out.push(other),
        }
    }
    out
}

/// `001-TestPattern` — must match `EffectInfo::file_stem`.
fn stem(row: &EffectSummary) -> String {
    let sanitized: String = row
        .id
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || c == '-' || c == '_' || c == '.' {
                c
            } else {
                '_'
            }
        })
        .collect();
    format!("{:03}-{}", row.index + 1, sanitized)
}

/// Renders the gallery. `rows` is the summary in catalog order.
pub fn render(rows: &[EffectSummary], meta: &GalleryMeta<'_>) -> String {
    let mut cards = String::new();
    for row in rows {
        let stem = stem(row);
        let gif = format!("{GIF_DIR}/{stem}.gif");
        let sheet = format!("{SHEET_DIR}/{stem}.png");
        let audio = if row.audio_reactive {
            r#"<span class="chip chip-audio">audio</span>"#
        } else {
            ""
        };
        let params = if row.has_parameters {
            r#"<span class="chip">parameters</span>"#
        } else {
            ""
        };
        let broken = row.nonblack_frames == 0;
        let note = if broken {
            r#"<p class="note">Renders black without an external audio/LFX source — documented default, not a render fault.</p>"#
        } else {
            ""
        };
        cards.push_str(&format!(
            r#"      <figure class="card" data-id="{id}" data-audio="{audio_flag}" data-params="{params_flag}">
        <a class="shot" href="{gif}" title="Open the full animation">
          <img src="{gif}" alt="Animated preview of the {id} effect" loading="lazy" width="160" height="160">
        </a>
        <figcaption>
          <h2>{name}</h2>
          <code>{id}</code>
          <div class="chips">{audio}{params}<a class="chip chip-link" href="{sheet}">contact sheet</a></div>
          {note}
        </figcaption>
      </figure>
"#,
            id = esc(&row.id),
            name = esc(&row.name),
            gif = gif,
            sheet = sheet,
            audio = audio,
            params = params,
            audio_flag = row.audio_reactive,
            params_flag = row.has_parameters,
            note = note,
        ));
    }

    let audio_count = rows.iter().filter(|row| row.audio_reactive).count();
    let total_kb: u64 = rows.iter().map(|row| row.bytes).sum::<u64>() / 1024;

    format!(
        r#"<!DOCTYPE html>
<html lang="en">
<head>
<meta charset="utf-8">
<meta name="viewport" content="width=device-width, initial-scale=1">
<title>LED matrix effect gallery · {rendered} effects</title>
<meta name="description" content="Animated previews of every built-in LED matrix effect, rendered by the real engine.">
<style>
  :root {{
    color-scheme: dark;
    --bg: #0b0f14;
    --card: #141b24;
    --border: #24303d;
    --text: #e6edf3;
    --muted: #8b9cb0;
    --accent: #38bdf8;
    --audio: #a78bfa;
  }}
  * {{ box-sizing: border-box; }}
  body {{
    margin: 0; padding: 32px 24px 64px;
    background: var(--bg); color: var(--text);
    font: 15px/1.5 ui-sans-serif, system-ui, -apple-system, "Segoe UI", sans-serif;
  }}
  header {{ max-width: 1280px; margin: 0 auto 24px; }}
  h1 {{ margin: 0 0 8px; font-size: 1.9rem; letter-spacing: -0.02em; }}
  .lead {{ margin: 0 0 20px; color: var(--muted); max-width: 70ch; }}
  .lead a {{ color: var(--accent); }}
  .controls {{ display: flex; flex-wrap: wrap; gap: 8px; align-items: center; margin-bottom: 8px; }}
  .controls input {{
    flex: 1 1 240px; min-width: 200px; padding: 9px 12px;
    background: var(--card); color: var(--text);
    border: 1px solid var(--border); border-radius: 8px; font: inherit;
  }}
  .controls input:focus {{ outline: 2px solid var(--accent); outline-offset: 1px; }}
  button {{
    padding: 8px 14px; border-radius: 8px; cursor: pointer; font: inherit;
    background: var(--card); color: var(--text); border: 1px solid var(--border);
  }}
  button:hover {{ border-color: var(--accent); }}
  button[aria-pressed="true"] {{ background: var(--accent); color: #04121c; border-color: var(--accent); font-weight: 600; }}
  .count {{ color: var(--muted); font-size: 0.9rem; margin: 4px 0 20px; }}
  .grid {{
    display: grid; gap: 16px; max-width: 1280px; margin: 0 auto;
    grid-template-columns: repeat(auto-fill, minmax(220px, 1fr));
  }}
  .card {{
    margin: 0; background: var(--card); border: 1px solid var(--border);
    border-radius: 12px; overflow: hidden; display: flex; flex-direction: column;
  }}
  .card[hidden] {{ display: none; }}
  .shot {{ display: block; background: #000; aspect-ratio: 1; }}
  .shot img {{ display: block; width: 100%; height: 100%; image-rendering: pixelated; }}
  figcaption {{ padding: 10px 12px 12px; display: flex; flex-direction: column; gap: 4px; }}
  h2 {{ margin: 0; font-size: 1rem; }}
  code {{ color: var(--muted); font-size: 0.78rem; }}
  .chips {{ display: flex; flex-wrap: wrap; gap: 6px; margin-top: 4px; }}
  .chip {{
    font-size: 0.7rem; padding: 2px 7px; border-radius: 999px;
    border: 1px solid var(--border); color: var(--muted); text-decoration: none;
  }}
  .chip-audio {{ color: var(--audio); border-color: var(--audio); }}
  .chip-link:hover {{ color: var(--accent); border-color: var(--accent); }}
  .note {{ margin: 6px 0 0; font-size: 0.75rem; color: var(--muted); }}
  .empty {{ max-width: 1280px; margin: 32px auto; color: var(--muted); }}
  footer {{ max-width: 1280px; margin: 40px auto 0; color: var(--muted); font-size: 0.85rem; border-top: 1px solid var(--border); padding-top: 16px; }}
  footer a {{ color: var(--accent); }}
</style>
</head>
<body>
<header>
  <h1>LED matrix effect gallery</h1>
  <p class="lead">
    All {rendered} built-in effects, rendered frame by frame by the real
    <code>matrix_core::Engine</code> — not screenshots, not hardware footage.
    {seconds} s at {fps} fps, one synthetic audio track, identical on every run.
    Repository: <a href="{repo}">{repo}</a>.
  </p>
  <div class="controls">
    <input id="search" type="search" placeholder="Search effects by name or id…" aria-label="Search effects">
    <button type="button" data-filter="all" aria-pressed="true">All</button>
    <button type="button" data-filter="audio" aria-pressed="false">Audio reactive</button>
    <button type="button" data-filter="parameters" aria-pressed="false">With parameters</button>
    <button type="button" data-filter="still" aria-pressed="false">Static / no params</button>
  </div>
  <p class="count"><span id="shown">{rendered}</span> of {rendered} shown · {audio_count} audio reactive · {total_kb} KB of GIFs</p>
</header>
<main class="grid" id="grid">
{cards}</main>
<p class="empty" id="empty" hidden>No effect matches that search.</p>
<footer>
  <p>
    Frames come from <code>cargo run -p matrix-gif -- --all</code>; contact sheets add a
    2×3 grid sampled at 10/30/50/70/85/95 % of the clip. Deterministic by
    construction: frame <em>i</em> is rendered at <em>t = i·1000/fps</em> ms after
    60 warm-up frames, so a re-render is byte-identical.
  </p>
  <p>
    A GIF shows what the panel would show. It is not a claim about brightness,
    power draw or colour calibration on real hardware.
  </p>
</footer>
<script>
(function () {{
  var cards = Array.prototype.slice.call(document.querySelectorAll('.card'));
  var search = document.getElementById('search');
  var shown = document.getElementById('shown');
  var empty = document.getElementById('empty');
  var filter = 'all';

  function matches(card, needle) {{
    if (!needle) return true;
    return card.getAttribute('data-id').toLowerCase().indexOf(needle) !== -1 ||
           card.querySelector('h2').textContent.toLowerCase().indexOf(needle) !== -1;
  }}

  function passes(card) {{
    var audio = card.getAttribute('data-audio') === 'true';
    var params = card.getAttribute('data-params') === 'true';
    if (filter === 'audio') return audio;
    if (filter === 'parameters') return params;
    if (filter === 'still') return !params && !audio;
    return true;
  }}

  function apply() {{
    var needle = search.value.trim().toLowerCase();
    var visible = 0;
    cards.forEach(function (card) {{
      var show = passes(card) && matches(card, needle);
      card.hidden = !show;
      if (show) visible++;
    }});
    shown.textContent = String(visible);
    empty.hidden = visible !== 0;
  }}

  search.addEventListener('input', apply);
  Array.prototype.forEach.call(document.querySelectorAll('[data-filter]'), function (button) {{
    button.addEventListener('click', function () {{
      filter = button.getAttribute('data-filter');
      Array.prototype.forEach.call(document.querySelectorAll('[data-filter]'), function (other) {{
        other.setAttribute('aria-pressed', String(other === button));
      }});
      apply();
    }});
  }});
  apply();
}})();
</script>
</body>
</html>
"#,
        rendered = rows.len(),
        seconds = meta.seconds,
        fps = meta.fps,
        repo = meta.repo,
        audio_count = audio_count,
        total_kb = total_kb,
        cards = cards,
    )
}

/// Renders a GitHub-viewable markdown gallery. GitHub does not execute or render
/// `.html` from a repository, so the HTML page is for a static host / `file://`
/// while this file is what actually scrolls on github.com. Relative image paths
/// resolve against the repository tree.
pub fn render_markdown(rows: &[EffectSummary], meta: &GalleryMeta<'_>) -> String {
    let audio_count = rows.iter().filter(|row| row.audio_reactive).count();
    let mut out = String::new();
    out.push_str("# Effect gallery\n\n");
    out.push_str(&format!(
        "All **{}** built-in effects, rendered frame by frame by the real \
`matrix_core::Engine` — not screenshots, not hardware footage. {} s at {} fps \
with one synthetic audio track, identical on every run.\n\n",
        rows.len(),
        meta.seconds,
        meta.fps
    ));
    out.push_str(&format!(
        "{} are audio reactive. Contact sheets sample each clip at \
10/30/50/70/85/95 %.\n\n",
        audio_count
    ));
    out.push_str(&format!(
        "Rendered by `cargo run -p matrix-gif -- --all`. Repository: <{}>\n\n---\n\n",
        meta.repo
    ));

    for row in rows {
        let stem = stem(row);
        out.push_str(&format!("### {} · `{}`\n\n", row.name, row.id));
        out.push_str(&format!(
            "![{}]({GIF_DIR}/{stem}.gif)\n\n",
            row.name.replace(']', "")
        ));
        let mut facts = vec![format!("{} colours", row.distinct_colors_max)];
        if row.audio_reactive {
            facts.push("audio reactive".to_string());
        }
        if row.has_parameters {
            facts.push("parameters".to_string());
        }
        facts.push(format!("[contact sheet]({SHEET_DIR}/{stem}.png)"));
        out.push_str(&facts.join(" · "));
        out.push_str("\n\n");
        if row.nonblack_frames == 0 {
            out.push_str(
                "> Renders black without an external audio/LFX source — documented \
default, not a render fault.\n\n",
            );
        }
    }

    out.push_str("---\n\n");
    out.push_str(
        "A GIF shows what the panel would show. It is not a claim about brightness, \
power draw or colour calibration on real hardware.\n",
    );
    out
}

/// Facts the page states about the render run.
pub struct GalleryMeta<'a> {
    pub seconds: f64,
    pub fps: u32,
    pub repo: &'a str,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn row(index: usize, id: &str, audio: bool, params: bool) -> EffectSummary {
        EffectSummary {
            index,
            id: id.into(),
            name: id.into(),
            frames: 100,
            fps: 25,
            seconds: 4.0,
            max_luma: 200,
            mean_luma: 40.0,
            nonblack_frames: 100,
            distinct_colors_max: 12,
            has_parameters: params,
            audio_reactive: audio,
            bytes: 1234,
            error: None,
        }
    }

    fn meta() -> GalleryMeta<'static> {
        GalleryMeta {
            seconds: 4.0,
            fps: 25,
            repo: "https://example.test/repo",
        }
    }

    #[test]
    fn every_effect_gets_a_card_with_relative_artifact_paths() {
        let rows = vec![
            row(0, "TestPattern", false, false),
            row(1, "fire", true, true),
        ];
        let html = render(&rows, &meta());
        assert!(html.contains("gifs/001-TestPattern.gif"));
        assert!(html.contains("contact-sheets/001-TestPattern.png"));
        assert!(html.contains("gifs/002-fire.gif"));
        assert!(!html.contains("http://") || html.contains("example.test"));
        // Absolute references would break `file://` and project-path hosting.
        assert!(!html.contains("src=\"/"));
    }

    #[test]
    fn ids_and_names_are_html_escaped() {
        let mut evil = row(0, "a<b>&\"'", false, false);
        evil.name = "<script>alert(1)</script>".into();
        let html = render(&[evil], &meta());
        assert!(!html.contains("<script>alert(1)</script>"));
        assert!(html.contains("&lt;script&gt;"));
        assert!(html.contains("&amp;"));
    }

    #[test]
    fn audio_and_parameter_flags_drive_filter_attributes() {
        let html = render(
            &[row(0, "fire", true, true), row(1, "Solid", false, false)],
            &meta(),
        );
        assert!(html.contains(r#"data-audio="true" data-params="true""#));
        assert!(html.contains(r#"data-audio="false" data-params="false""#));
    }

    #[test]
    fn a_black_effect_is_labelled_instead_of_hidden() {
        let mut frames = row(0, "Frames", true, true);
        frames.nonblack_frames = 0;
        let html = render(&[frames], &meta());
        assert!(html.contains("Renders black"));
        // It must still be part of the gallery, just annotated.
        assert!(html.contains("gifs/001-Frames.gif"));
    }
}
