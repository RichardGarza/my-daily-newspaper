//! The paper paper.
//!
//! edition -> print-layout HTML (this file) -> PDF (headless Chrome) -> printer (`lp`).
//!
//! Why Chrome: paginated multi-column typesetting with pictures is exactly what
//! a browser engine already does, and one is installed. The app's own WebKit
//! view can only print through a dialog, and the scheduled run has no window.
//! Why `lp`: it is the macOS / CUPS print command - no dialog, queues the job
//! if the printer is asleep, works from a background process.
//!
//! You can't click paper, so every story carries a small QR code where the
//! screen edition has its "Read more" / "See the video" button.

use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::time::Duration;

use base64::Engine as _;
use chrono::{Local, NaiveDate};
use serde::Serialize;
use tauri::AppHandle;

use crate::model::{Card, Edition, Settings};
use crate::scores::{self, ScoreBug};
use crate::{paths, schedule, store, wire};

const MASTHEAD_FONT: &[u8] = include_bytes!("../../src/assets/fonts/Chomsky.woff2");

// ------------------------------------------------------------------- status

#[derive(Serialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct PrintStatus {
    /// Print automatically after the scheduled morning refresh.
    pub print_daily: bool,
    /// A Chromium-family browser was found to make the PDF.
    pub browser_found: bool,
    /// The printer that would be used, if one can be determined.
    pub printer: Option<String>,
    /// Why printing can't work right now, if it can't.
    pub problem: Option<String>,
}

pub async fn status(app: &AppHandle) -> PrintStatus {
    let settings = store::load_settings(app);
    let browser_found = find_browser(&settings).is_some();
    let printer = pick_printer(&settings).await;
    let problem = if !browser_found {
        Some("Printing needs Google Chrome (or Edge / Brave) installed to make the PDF.".to_string())
    } else {
        printer.as_ref().err().cloned()
    };
    PrintStatus { print_daily: settings.print_daily, browser_found, printer: printer.ok(), problem }
}

// --------------------------------------------------------------------- HTML

const CSS: &str = r#"
@page { size: Letter; margin: 0.5in 0.55in 0.6in; }
* { box-sizing: border-box; }
html { -webkit-print-color-adjust: exact; print-color-adjust: exact; }
body { margin: 0; color: #111; font: 9.4pt/1.33 Georgia, "Times New Roman", "Liberation Serif", serif; }
a { color: inherit; text-decoration: none; }

.masthead { display: grid; grid-template-columns: 1.55in 1fr 1.55in; align-items: center; gap: 0.15in; }
.nameplate { font-family: Chomsky, "Old English Text MT", Georgia, serif; font-size: 46pt; line-height: 1; text-align: center; white-space: nowrap; margin: 0; font-weight: 400; }
.ear { border: 0.6pt solid #111; padding: 5pt 6pt; font-size: 7.4pt; line-height: 1.3; }
.ear b { display: block; font-size: 8.4pt; }
.ear .lab, .sans { font-family: "Helvetica Neue", Helvetica, Arial, sans-serif; text-transform: uppercase; letter-spacing: 0.09em; }
.ear .lab { font-size: 6.2pt; border-bottom: 0.4pt solid #999; padding-bottom: 2pt; margin-bottom: 3pt; display: block; }
.ear i { color: #333; }
.dateline { display: grid; grid-template-columns: 1fr auto 1fr; gap: 10pt; align-items: baseline; margin-top: 7pt; padding: 3pt 0 3.5pt; border-top: 0.6pt solid #111; border-bottom: 2.2pt double #111; font-size: 6.8pt; }
.dateline .c { text-align: center; font-weight: 700; }
.dateline .r { text-align: right; font-family: Georgia, serif; text-transform: none; letter-spacing: 0; font-style: italic; font-size: 8pt; }

h2.desk { font-family: "Helvetica Neue", Helvetica, Arial, sans-serif; font-size: 7.6pt; font-weight: 700; letter-spacing: 0.2em; text-transform: uppercase; margin: 13pt 0 8pt; padding: 3pt 0 2.5pt; border-top: 2.2pt double #111; border-bottom: 0.6pt solid #111; break-after: avoid; }

.lead { margin-top: 10pt; }
.lead .top { display: grid; grid-template-columns: 1.08fr 1fr; gap: 0.2in; align-items: start; }
.lead.noimg .top { grid-template-columns: 1fr; }
.lead h1 { font-size: 23pt; line-height: 1.06; margin: 0 0 6pt; letter-spacing: -0.01em; }
.lead .dek { font-size: 11pt; line-height: 1.32; }
.lead .copy { column-count: 3; column-gap: 0.2in; column-rule: 0.4pt solid #aaa; margin-top: 8pt; font-size: 9.8pt; }
.lead .copy p:first-child::first-letter { float: left; font-weight: 700; font-size: 3.1em; line-height: 0.82; padding: 3pt 4pt 0 0; }

.cols { column-count: 3; column-gap: 0.2in; column-rule: 0.4pt solid #aaa; }
.story { margin: 0 0 11pt; padding: 0 0 9pt; border-bottom: 0.4pt solid #bbb; }
.story:last-child { border-bottom: 0; }
.story h3 { font-size: 12.2pt; line-height: 1.12; margin: 0 0 3pt; break-after: avoid; }
.story.brief h3 { font-size: 10.6pt; }
.kicker { font-family: "Helvetica Neue", Helvetica, Arial, sans-serif; font-size: 6.2pt; letter-spacing: 0.14em; text-transform: uppercase; color: #444; margin-bottom: 2pt; break-after: avoid; }
.dek { color: #222; margin: 0 0 4pt; font-size: 9.6pt; line-height: 1.3; break-after: avoid; }
.byline { font-family: "Helvetica Neue", Helvetica, Arial, sans-serif; font-size: 6.2pt; letter-spacing: 0.09em; text-transform: uppercase; color: #555; padding-bottom: 3pt; margin-bottom: 4pt; border-bottom: 0.4pt solid #ccc; break-after: avoid; }
.byline b { color: #111; font-weight: 600; }
.copy p { margin: 0 0 3pt; text-align: justify; hyphens: auto; -webkit-hyphens: auto; orphans: 2; widows: 2; }
.copy p + p { text-indent: 1.1em; }
.quote { font-style: italic; font-size: 10pt; line-height: 1.33; margin: 0 0 4pt; }
.quote::before { content: "\201C"; font-style: normal; font-weight: 700; font-size: 17pt; line-height: 0; vertical-align: -5pt; margin-right: 1pt; }

figure { margin: 0 0 6pt; break-inside: avoid; }
figure img { display: block; width: 100%; aspect-ratio: 16 / 9; object-fit: cover; }
.bw figure img { filter: grayscale(1) contrast(1.06); }
.vid figure { position: relative; }
.vid figure::after { content: "\25B6"; position: absolute; left: 5pt; bottom: 5pt; width: 15pt; height: 15pt; background: #111; color: #fff; font: 8pt/15pt Arial, sans-serif; text-align: center; padding-left: 1pt; }

.more { display: flex; align-items: center; gap: 6pt; margin-top: 5pt; break-inside: avoid; }
.more + .more { margin-top: 4pt; }
.qr svg { display: block; width: 0.44in; height: 0.44in; }
.more .txt { font-family: "Helvetica Neue", Helvetica, Arial, sans-serif; font-size: 6.4pt; line-height: 1.35; min-width: 0; }
.more .txt b { display: block; text-transform: uppercase; letter-spacing: 0.1em; color: #111; font-weight: 600; }

.notes { margin-top: 12pt; padding-top: 5pt; border-top: 0.6pt solid #111; font-size: 7.2pt; color: #555; }
.notes b { font-family: "Helvetica Neue", Helvetica, Arial, sans-serif; font-size: 6.2pt; letter-spacing: 0.14em; text-transform: uppercase; color: #111; }
.colophon { margin-top: 8pt; padding-top: 4pt; border-top: 2.2pt double #111; font-size: 6.2pt; color: #555; display: flex; justify-content: space-between; }
"#;

pub struct RenderOptions {
    pub color: bool,
    pub qr: bool,
    /// "Sam's Daily"
    pub paper: String,
    /// Dateline city; empty = none.
    pub city: String,
    /// Last game and next game for the owner's team, when there is one.
    pub score: Option<ScoreBug>,
}

impl RenderOptions {
    #[cfg(test)]
    fn plain(color: bool, qr: bool) -> Self {
        Self { color, qr, paper: "Priya\u{2019}s Daily".into(), city: "Boise".into(), score: None }
    }
}

/// "PDT" - chrono can't name the local zone, `date` can.
fn zone_abbrev() -> String {
    std::process::Command::new("date")
        .arg("+%Z")
        .output()
        .ok()
        .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
        .filter(|z| !z.is_empty() && z.len() <= 5)
        .unwrap_or_default()
}

fn day_label(when: chrono::DateTime<Local>) -> String {
    let days = (when.date_naive() - Local::now().date_naive()).num_days();
    match days {
        0 => "Today".into(),
        1 => "Tomorrow".into(),
        -1 => "Yesterday".into(),
        2..=6 => when.format("%A").to_string(),
        _ => when.format("%a, %b %-d").to_string(),
    }
}

fn local(iso: &str) -> Option<chrono::DateTime<Local>> {
    chrono::DateTime::parse_from_rfc3339(iso).ok().map(|t| t.with_timezone(&Local))
}

/// Two lines for the masthead: what happened, and what's next and where to watch.
pub fn scoreboard_lines(score: &ScoreBug, zone: &str) -> Vec<String> {
    let mut lines = Vec::new();
    if let Some(g) = &score.game {
        let pts = |n: Option<u32>| n.map(|v| v.to_string()).unwrap_or_else(|| "-".into());
        let day = local(&g.start).map(day_label).unwrap_or_default();
        lines.push(format!("{}, {}: {} {}, {} {}", g.detail, day, g.away.name, pts(g.away.score), g.home.name, pts(g.home.score)));
    }
    if let Some(n) = &score.next {
        let (us, them, joiner) = if n.we_are_home { (&n.home.name, &n.away.name, "vs.") } else { (&n.away.name, &n.home.name, "at") };
        let when = local(&n.start)
            .map(|t| format!("{} {}{}", day_label(t), t.format("%-I:%M %p"), if zone.is_empty() { String::new() } else { format!(" {zone}") }))
            .unwrap_or_default();
        let tv = if n.tv.is_empty() { String::new() } else { format!(" \u{00b7} {}", n.tv.join(", ")) };
        lines.push(format!("Next: {us} {joiner} {them} \u{00b7} {when}{tv}"));
    }
    lines
}

pub fn esc(s: &str) -> String {
    s.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;").replace('"', "&quot;")
}

fn long_date(date: &str) -> String {
    NaiveDate::parse_from_str(date, "%Y-%m-%d")
        .map(|d| d.format("%A, %B %-d, %Y").to_string())
        .unwrap_or_else(|_| date.to_string())
}

/// "Sept. 19"-style date for a byline; relative times mean nothing on paper.
fn short_date(iso: Option<&str>) -> Option<String> {
    let d = wire::parse_date(iso?)?.with_timezone(&Local);
    Some(d.format("%b %-d").to_string())
}

fn roman(mut n: u32) -> String {
    let table = [(1000, "M"), (900, "CM"), (500, "D"), (400, "CD"), (100, "C"), (90, "XC"), (50, "L"), (40, "XL"), (10, "X"), (9, "IX"), (5, "V"), (4, "IV"), (1, "I")];
    let mut out = String::new();
    n = n.max(1);
    for (v, s) in table {
        while n >= v {
            out.push_str(s);
            n -= v;
        }
    }
    out
}

fn paragraphs(text: Option<&str>) -> Vec<String> {
    text.unwrap_or("")
        .split("\n\n")
        .map(|p| p.trim().to_string())
        .filter(|p| !p.is_empty())
        .collect()
}

fn qr_svg(url: &str) -> Option<String> {
    use qrcode::render::svg;
    use qrcode::{EcLevel, QrCode};
    let code = QrCode::with_error_correction_level(url.as_bytes(), EcLevel::L).ok()?;
    let svg = code.render::<svg::Color>().min_dimensions(90, 90).quiet_zone(false).build();
    // Inline SVG: drop the XML declaration.
    Some(match svg.find("<svg") {
        Some(i) => svg[i..].to_string(),
        None => svg,
    })
}

/// Where a link goes, in words: "YouTube", "X", "techcrunch.com". The address
/// itself lives in the QR code; nobody types a URL off paper.
fn site_name(url: &str) -> String {
    let host = wire::host_label(url);
    match host.as_str() {
        "youtube.com" | "youtu.be" | "m.youtube.com" => "YouTube".into(),
        "x.com" | "twitter.com" => "X".into(),
        _ => host,
    }
}

fn more_block(label: &str, url: &str, opts: &RenderOptions) -> String {
    let qr = if opts.qr { qr_svg(url).map(|s| format!("<div class=\"qr\">{s}</div>")).unwrap_or_default() } else { String::new() };
    let site = site_name(url);
    let on = if site.is_empty() { String::new() } else { format!(" on {}", esc(&site)) };
    format!("<div class=\"more\">{qr}<div class=\"txt\"><b>{}{on}</b></div></div>", esc(label))
}

fn links(card: &Card, opts: &RenderOptions) -> String {
    let primary = match card.kind.as_str() {
        "video" => "See the video",
        "post" => "See the post",
        _ => "Read more",
    };
    let mut out = more_block(primary, &card.url, opts);
    match (card.kind.as_str(), &card.video_url, &card.article_url) {
        ("article", Some(v), _) => out.push_str(&more_block("See the video", v, opts)),
        ("video", _, Some(a)) => out.push_str(&more_block("Read more", a, opts)),
        _ => {}
    }
    out
}

fn figure(card: &Card) -> String {
    match &card.image {
        Some(src) if src.starts_with("http") => format!(
            "<figure><img src=\"{}\" alt=\"\" referrerpolicy=\"no-referrer\" onerror=\"this.parentNode.style.display='none'\"></figure>",
            esc(src)
        ),
        _ => String::new(),
    }
}

fn byline(card: &Card) -> String {
    let who = if card.kind == "post" { card.author.clone().unwrap_or_else(|| "X".into()) } else { card.source.clone() };
    let when = short_date(card.published.as_deref()).map(|d| format!(" &nbsp;·&nbsp; {}", esc(&d))).unwrap_or_default();
    format!("<div class=\"byline\"><b>{}</b>{when}</div>", esc(&who))
}

fn copy_html(card: &Card) -> String {
    let paras = paragraphs(card.story.as_deref());
    if paras.is_empty() {
        return String::new();
    }
    let body: String = paras.iter().map(|p| format!("<p>{}</p>", esc(p))).collect();
    format!("<div class=\"copy\">{body}</div>")
}

fn story_html(card: &Card, opts: &RenderOptions) -> String {
    let has_copy = !paragraphs(card.story.as_deref()).is_empty();
    if card.kind == "post" {
        return format!(
            "<article class=\"story post\"><div class=\"kicker\">On X</div><p class=\"quote\">{}</p>{}{}{}</article>",
            esc(if card.dek.is_empty() { &card.headline } else { &card.dek }),
            byline(card),
            copy_html(card),
            links(card, opts)
        );
    }
    let is_video = card.kind == "video";
    // Pictures: features always, briefs only when it's a video.
    let fig = if card.size != "brief" || is_video { figure(card) } else { String::new() };
    let dek = if !card.dek.is_empty() && (!has_copy || card.size != "brief") {
        format!("<p class=\"dek\">{}</p>", esc(&card.dek))
    } else {
        String::new()
    };
    format!(
        "<article class=\"story {size}{vid}\">{fig}{kicker}<h3>{headline}</h3>{dek}{byline}{copy}{links}</article>",
        size = esc(&card.size),
        vid = if is_video { " vid" } else { "" },
        kicker = if is_video { "<div class=\"kicker\">Video</div>" } else { "" },
        headline = esc(&card.headline),
        byline = byline(card),
        copy = copy_html(card),
        links = links(card, opts),
    )
}

fn lead_html(card: &Card, opts: &RenderOptions) -> String {
    let fig = figure(card);
    let is_video = card.kind == "video";
    format!(
        "<section class=\"lead{noimg}{vid}\"><div class=\"top\">{fig}<div>{kicker}<h1>{headline}</h1>{dek}{byline}</div></div>{copy}{links}</section>",
        noimg = if fig.is_empty() { " noimg" } else { "" },
        vid = if is_video { " vid" } else { "" },
        kicker = if is_video { "<div class=\"kicker\">Video</div>" } else { "" },
        headline = esc(&card.headline),
        dek = if card.dek.is_empty() { String::new() } else { format!("<p class=\"dek\">{}</p>", esc(&card.dek)) },
        byline = byline(card),
        copy = copy_html(card),
        links = links(card, opts),
    )
}

/// The whole print edition as one self-contained HTML document.
pub fn render_html(edition: &Edition, opts: &RenderOptions) -> String {
    let font = base64::engine::general_purpose::STANDARD.encode(MASTHEAD_FONT);
    let year: u32 = edition.date.get(0..4).and_then(|y| y.parse().ok()).unwrap_or(2026);
    let date = long_date(&edition.date);
    let printed = chrono::DateTime::parse_from_rfc3339(&edition.generated_at)
        .map(|t| t.with_timezone(&Local).format("%-I:%M %p").to_string())
        .unwrap_or_default();

    let mut html = String::with_capacity(64_000);
    html.push_str("<!doctype html><html lang=\"en\"><head><meta charset=\"utf-8\">");
    html.push_str(&format!("<title>{} - {}</title>", esc(&opts.paper), esc(&edition.date)));
    html.push_str("<style>@font-face{font-family:Chomsky;src:url(data:font/woff2;base64,");
    html.push_str(&font);
    html.push_str(") format(\"woff2\");}");
    html.push_str(CSS);
    html.push_str("</style></head>");
    html.push_str(if opts.color { "<body>" } else { "<body class=\"bw\">" });

    // masthead
    let right_ear = match opts.score.as_ref().map(|sc| scoreboard_lines(sc, &zone_abbrev())).filter(|l| !l.is_empty()) {
        Some(lines) => format!(
            "<div class=\"ear\"><span class=\"lab\">Scoreboard</span>{}</div>",
            lines.iter().enumerate().map(|(i, l)| if i == 0 { format!("<b>{}</b>", esc(l)) } else { format!("<i>{}</i>", esc(l)) }).collect::<String>()
        ),
        None => format!(
            "<div class=\"ear\"><span class=\"lab\">Press Room</span><b>{n} stories</b><i>Printed {printed} · {s} searches</i></div>",
            n = edition.cards.len(),
            printed = esc(&printed),
            s = edition.stats.searches,
        ),
    };
    html.push_str(&format!(
        "<header class=\"masthead\"><div class=\"ear\"><span class=\"lab\">Today's Edition</span><b>{date}</b><i>{n} stories · printed {printed}</i></div>\
         <h1 class=\"nameplate\">{paper}</h1>{right_ear}</header>",
        date = esc(&date),
        n = edition.cards.len(),
        printed = esc(&printed),
        paper = esc(&opts.paper),
    ));
    let place = if opts.city.trim().is_empty() { date.clone() } else { format!("{}, {}", opts.city.trim(), date) };
    html.push_str(&format!(
        "<div class=\"dateline sans\"><span>Vol. {} . . . No. {}</span><span class=\"c\">{}</span><span class=\"r\">{}</span></div>",
        roman(year.saturating_sub(2025)),
        edition.edition_no,
        esc(&place),
        edition.tagline.as_deref().map(|t| format!("\u{201C}{}\u{201D}", esc(t))).unwrap_or_default(),
    ));

    // lead, then each desk in the editor's order
    let lead = edition.cards.iter().find(|c| c.size == "lead").or_else(|| edition.cards.first());
    if let Some(l) = lead {
        html.push_str(&lead_html(l, opts));
    }
    let lead_id = lead.map(|l| l.id.clone()).unwrap_or_default();

    let mut sections: Vec<String> = edition.sections.clone();
    for c in &edition.cards {
        if !sections.contains(&c.section) {
            sections.push(c.section.clone());
        }
    }
    for section in &sections {
        let mut cards: Vec<&Card> = edition.cards.iter().filter(|c| &c.section == section && c.id != lead_id).collect();
        if cards.is_empty() {
            continue;
        }
        cards.sort_by_key(|c| if c.size == "brief" { 1 } else { 0 });
        html.push_str(&format!("<h2 class=\"desk\">{}</h2><div class=\"cols\">", esc(section)));
        for c in cards {
            html.push_str(&story_html(c, opts));
        }
        html.push_str("</div>");
    }

    html.push_str(&format!(
        "<div class=\"colophon sans\"><span>{} · printed on demand · one subscriber</span><span>{}</span></div>",
        esc(&opts.paper),
        esc(&edition.date)
    ));
    html.push_str("</body></html>");
    html
}

// ---------------------------------------------------------------------- PDF

/// A Chromium-family browser that can run headless.
pub fn find_browser(settings: &Settings) -> Option<PathBuf> {
    let o = settings.chrome_bin.trim();
    if !o.is_empty() {
        let p = paths::expand_tilde(o);
        return p.is_file().then_some(p);
    }
    let home = std::env::var_os("HOME").map(PathBuf::from);
    let mac_apps = [
        "Google Chrome.app/Contents/MacOS/Google Chrome",
        "Microsoft Edge.app/Contents/MacOS/Microsoft Edge",
        "Brave Browser.app/Contents/MacOS/Brave Browser",
        "Chromium.app/Contents/MacOS/Chromium",
        "Vivaldi.app/Contents/MacOS/Vivaldi",
    ];
    let mut candidates: Vec<PathBuf> = Vec::new();
    for app in mac_apps {
        candidates.push(Path::new("/Applications").join(app));
        if let Some(h) = &home {
            candidates.push(h.join("Applications").join(app));
        }
    }
    for bin in ["/usr/bin/google-chrome", "/usr/bin/chromium", "/usr/bin/chromium-browser"] {
        candidates.push(PathBuf::from(bin));
    }
    candidates.into_iter().find(|p| p.is_file())
}

fn print_dir(app: &AppHandle) -> Result<PathBuf, String> {
    let dir = store::data_dir(app)?.join("print");
    std::fs::create_dir_all(&dir).map_err(|e| format!("Couldn't create {}: {e}", dir.display()))?;
    Ok(dir)
}

/// Render the edition and have the browser print it to a PDF. Returns the PDF path.
pub async fn make_pdf(app: &AppHandle, edition: &Edition) -> Result<PathBuf, String> {
    let settings = store::load_settings(app);
    let browser = find_browser(&settings)
        .ok_or("Printing needs Google Chrome (or Edge / Brave) to make the PDF, and I couldn't find one. Install Chrome, or set chromeBin in settings.json.")?;
    let dir = print_dir(app)?;
    let html_path = dir.join(format!("{}.html", edition.date));
    let pdf_path = dir.join(format!("My Daily {}.pdf", edition.date));

    let score = if settings.mlb_team_id == 0 {
        None
    } else {
        tokio::time::timeout(Duration::from_secs(8), scores::fetch(&wire::http_client(), settings.mlb_team_id)).await.ok().and_then(|r| r.ok())
    };
    let opts = RenderOptions {
        color: settings.print_color,
        qr: settings.print_qr,
        paper: store::paper_name(&store::owner_name(&settings)),
        city: settings.city.clone(),
        score,
    };
    let html = render_html(edition, &opts);
    std::fs::write(&html_path, html).map_err(|e| format!("Couldn't write {}: {e}", html_path.display()))?;
    let _ = std::fs::remove_file(&pdf_path);

    let file_url = url::Url::from_file_path(&html_path).map_err(|_| "Couldn't build a file:// address for the print page.".to_string())?;
    // A private profile: without it, a headless launch just pokes the Chrome
    // that's already open and exits without printing anything.
    let profile = dir.join("browser-profile");

    let mut cmd = tokio::process::Command::new(&browser);
    cmd.arg("--headless=new")
        .arg("--disable-gpu")
        .arg("--no-first-run")
        .arg("--no-default-browser-check")
        .arg("--disable-extensions")
        .arg("--hide-scrollbars")
        .arg("--no-pdf-header-footer")
        .arg("--print-to-pdf-no-header")
        .arg("--virtual-time-budget=25000")
        .arg(format!("--user-data-dir={}", profile.display()))
        .arg(format!("--print-to-pdf={}", pdf_path.display()))
        .arg(file_url.as_str())
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::piped())
        .kill_on_drop(true);
    if cfg!(target_os = "linux") {
        cmd.arg("--no-sandbox");
    }

    let out = tokio::time::timeout(Duration::from_secs(90), cmd.output())
        .await
        .map_err(|_| "The browser took more than 90 seconds to make the PDF.".to_string())?
        .map_err(|e| format!("Couldn't start {}: {e}", browser.display()))?;

    let size = std::fs::metadata(&pdf_path).map(|m| m.len()).unwrap_or(0);
    if size < 2_000 {
        let err = String::from_utf8_lossy(&out.stderr);
        let tail: Vec<&str> = err.lines().rev().take(3).collect();
        return Err(format!("The browser didn't produce a PDF. {}", wire::truncate(&tail.join(" | "), 300)));
    }

    // Housekeeping: keep two weeks of print files.
    if let Ok(rd) = std::fs::read_dir(&dir) {
        for e in rd.flatten() {
            let old = e.metadata().ok().and_then(|m| m.modified().ok()).and_then(|t| t.elapsed().ok()).map(|a| a.as_secs() > 14 * 86_400).unwrap_or(false);
            if old && e.path().is_file() {
                let _ = std::fs::remove_file(e.path());
            }
        }
    }
    Ok(pdf_path)
}

// ------------------------------------------------------------------ printer

async fn run(cmd: &str, args: &[&str]) -> Result<(bool, String, String), String> {
    let out = tokio::time::timeout(
        Duration::from_secs(20),
        tokio::process::Command::new(cmd).args(args).stdin(Stdio::null()).output(),
    )
    .await
    .map_err(|_| format!("{cmd} didn't answer."))?
    .map_err(|e| format!("Couldn't run {cmd}: {e}"))?;
    Ok((
        out.status.success(),
        String::from_utf8_lossy(&out.stdout).to_string(),
        String::from_utf8_lossy(&out.stderr).to_string(),
    ))
}

/// "system default destination: HP_LaserJet" -> "HP_LaserJet"
pub fn parse_default_printer(lpstat_d: &str) -> Option<String> {
    let line = lpstat_d.lines().find(|l| l.contains("default destination:"))?;
    let name = line.split(':').nth(1)?.trim();
    (!name.is_empty()).then(|| name.to_string())
}

/// The printer to use: settings.json -> system default -> the only printer there is.
pub async fn pick_printer(settings: &Settings) -> Result<String, String> {
    let named = settings.printer.trim();
    if !named.is_empty() {
        return Ok(named.to_string());
    }
    let (_, out, _) = run("lpstat", &["-d"]).await?;
    if let Some(p) = parse_default_printer(&out) {
        return Ok(p);
    }
    // macOS set to "Last Printer Used" reports no default. If there's exactly
    // one printer, that's the one.
    let (_, list, _) = run("lpstat", &["-e"]).await?;
    let printers: Vec<&str> = list.lines().map(|l| l.trim()).filter(|l| !l.is_empty()).collect();
    match printers.as_slice() {
        [] => Err("No printer is set up on this Mac (System Settings > Printers & Scanners).".into()),
        [only] => Ok(only.to_string()),
        many => Err(format!(
            "There are {} printers and no default. Pick a default in System Settings > Printers & Scanners, or put one of these in settings.json as \"printer\": {}",
            many.len(),
            many.join(", ")
        )),
    }
}

pub fn lp_args(printer: &str, pdf: &Path, title: &str, settings: &Settings) -> Vec<String> {
    let mut a = vec!["-d".to_string(), printer.to_string(), "-t".to_string(), title.to_string()];
    if settings.print_max_pages > 0 {
        a.push("-o".into());
        a.push(format!("page-ranges=1-{}", settings.print_max_pages));
    }
    a.push("-o".into());
    a.push(if settings.print_duplex { "sides=two-sided-long-edge" } else { "sides=one-sided" }.into());
    a.push("-o".into());
    a.push("media=Letter".into());
    a.push(pdf.display().to_string());
    a
}

/// Queue the PDF on the printer. Returns lp's confirmation ("request id is ...").
pub async fn send_to_printer(app: &AppHandle, pdf: &Path, date: &str) -> Result<String, String> {
    let settings = store::load_settings(app);
    let printer = pick_printer(&settings).await?;
    let title = format!("{} {date}", store::paper_name(&store::owner_name(&settings)));
    let args = lp_args(&printer, pdf, &title, &settings);
    let refs: Vec<&str> = args.iter().map(|s| s.as_str()).collect();
    let (ok, out, err) = run("lp", &refs).await?;
    if !ok {
        return Err(format!("The printer \"{printer}\" refused the job: {}", wire::truncate(err.trim(), 240)));
    }
    Ok(format!("{printer}: {}", out.trim()))
}

// ------------------------------------------------------------- morning print

fn printed_marker(app: &AppHandle, date: &str) -> Option<PathBuf> {
    Some(print_dir(app).ok()?.join(format!("{date}.printed")))
}

/// Minutes between the scheduled delivery time and now; None if delivery is off.
fn minutes_past_schedule() -> Option<i64> {
    use chrono::Timelike;
    let s = schedule::status();
    if !s.enabled {
        return None;
    }
    let now = Local::now();
    let now_min = (now.hour() * 60 + now.minute()) as i64;
    let sched = (s.hour * 60 + s.minute) as i64;
    Some((now_min - sched).rem_euclid(24 * 60))
}

/// Called by the scheduled background run once today's edition exists.
/// Prints at most once a day, and not when the "morning" job actually ran in
/// the evening because the Mac was asleep all day.
pub async fn morning_print(app: &AppHandle, edition: &Edition) -> Option<String> {
    let settings = store::load_settings(app);
    if !settings.print_daily {
        return None;
    }
    let marker = printed_marker(app, &edition.date)?;
    if marker.exists() {
        return Some("Already printed today.".into());
    }
    if let Some(late) = minutes_past_schedule() {
        if late > 6 * 60 {
            return Some(format!("Not printing: this run is {} hours past the delivery time. Use the Print button if you still want paper.", late / 60));
        }
    }
    let result = async {
        let pdf = make_pdf(app, edition).await?;
        send_to_printer(app, &pdf, &edition.date).await
    }
    .await;
    Some(match result {
        Ok(msg) => {
            let _ = std::fs::write(&marker, &msg);
            format!("Sent to printer - {msg}")
        }
        Err(e) => format!("Printing failed: {e}"),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::EditionStats;

    fn card(id: &str, kind: &str, size: &str, section: &str) -> Card {
        Card {
            id: id.into(),
            kind: kind.into(),
            size: size.into(),
            section: section.into(),
            headline: format!("Headline {id} <script>alert(1)</script>"),
            dek: "A \"quoted\" standfirst & more.".into(),
            story: Some("First paragraph.\n\nSecond paragraph.".into()),
            source: "Source & Co".into(),
            author: None,
            url: format!("https://example.com/{id}"),
            video_url: None,
            article_url: None,
            image: None,
            published: Some("2026-09-19T17:00:00Z".into()),
            why: None,
        }
    }

    fn edition() -> Edition {
        let mut lead = card("lead", "article", "lead", "Front Page");
        lead.video_url = Some("https://www.youtube.com/watch?v=abcdefghijk".into());
        lead.image = Some("https://img.example.com/a.jpg".into());
        Edition {
            date: "2026-09-20".into(),
            generated_at: "2026-09-20T06:02:00-07:00".into(),
            edition_no: 3,
            tagline: Some("All the news that's fit to print, literally".into()),
            sections: vec!["Front Page".into(), "Tech".into()],
            cards: vec![lead, card("f1", "video", "feature", "Front Page"), card("b1", "article", "brief", "Tech"), card("p1", "post", "brief", "Tech")],
            notes: vec![],
            stats: EditionStats::default(),
        }
    }

    #[test]
    fn renders_every_story_escaped_with_qr_codes() {
        let html = render_html(&edition(), &RenderOptions::plain(false, true));
        assert!(html.contains("Boise, Sunday, September 20, 2026"));
        assert!(html.contains("Priya\u{2019}s Daily</h1>"));
        assert!(!html.contains("Richard"));
        assert!(html.contains("Vol. I . . . No. 3"));
        assert!(html.contains("<body class=\"bw\">"));
        assert!(!html.contains("<script>alert(1)</script>"));
        assert!(html.contains("&lt;script&gt;"));
        assert!(html.contains("Source &amp; Co"));
        for id in ["lead", "f1", "b1"] {
            assert!(html.contains(&format!("Headline {id} ")), "missing {id}");
        }
        assert!(html.contains("Read more on example.com"));
        assert!(html.contains("See the video on YouTube"));
        assert!(!html.contains("example.com/lead"), "raw URLs don't belong on paper");
        // 4 primary links + the lead's video link
        assert_eq!(html.matches("<div class=\"qr\"><svg").count(), 5);
        assert!(html.contains("See the video"));
        assert!(html.contains("On X"));
        assert!(html.contains("<h2 class=\"desk\">Tech</h2>"));
        // the lead is not repeated inside its desk
        assert_eq!(html.matches("Headline lead").count(), 1);
    }

    #[test]
    fn qr_and_colour_are_optional() {
        let html = render_html(&edition(), &RenderOptions::plain(true, false));
        assert!(html.contains("<body>"));
        assert_eq!(html.matches("class=\"qr\"").count(), 0);
        assert!(html.contains("Read more"));
    }

    #[test]
    fn printer_and_lp_arguments() {
        assert_eq!(parse_default_printer("system default destination: HP_LaserJet_M110w\n").as_deref(), Some("HP_LaserJet_M110w"));
        assert_eq!(parse_default_printer("no system default destination\n"), None);
        let s = Settings::default();
        let a = lp_args("HP", Path::new("/tmp/x y.pdf"), "Sam's Daily 2026-09-20", &s);
        assert_eq!(a[0..2], ["-d".to_string(), "HP".to_string()]);
        assert!(a.contains(&"page-ranges=1-8".to_string()));
        assert!(a.contains(&"sides=two-sided-long-edge".to_string()));
        assert_eq!(a.last().unwrap(), "/tmp/x y.pdf");
    }

    #[test]
    fn scoreboard_says_what_happened_and_what_is_next() {
        use crate::scores::{GameLine, TeamLine};
        let team = |abbr: &str, name: &str, score: Option<u32>| TeamLine { abbr: abbr.into(), name: name.into(), score };
        let last = GameLine { game_pk: 1, state: "final".into(), detail: "Final/10".into(), start: "2020-01-01T02:10:00Z".into(), away: team("SF", "Giants", Some(3)), home: team("LAD", "Dodgers", Some(5)), we_are_home: true, url: String::new(), tv: vec![], situation: None };
        let next = GameLine { game_pk: 2, state: "preview".into(), detail: "Scheduled".into(), start: "2020-01-02T02:10:00Z".into(), away: team("LAD", "Dodgers", None), home: team("SD", "Padres", None), we_are_home: false, url: String::new(), tv: vec!["SportsNet LA".into(), "Apple TV+".into()], situation: None };
        let lines = scoreboard_lines(&ScoreBug { game: Some(last), next: Some(next) }, "PST");
        assert_eq!(lines.len(), 2);
        assert!(lines[0].starts_with("Final/10, "));
        assert!(lines[0].ends_with("Giants 3, Dodgers 5"));
        assert!(lines[1].starts_with("Next: Dodgers at Padres \u{00b7} "));
        assert!(lines[1].contains(" PST \u{00b7} SportsNet LA, Apple TV+"));
    }

    /// Proofing aid: RD_EDITION=/path/edition.json RD_OUT=/path/out.html
    ///   cargo test --lib print_proof -- --ignored
    #[test]
    #[ignore]
    fn print_proof() {
        let src = std::env::var("RD_EDITION").expect("RD_EDITION");
        let out = std::env::var("RD_OUT").expect("RD_OUT");
        let edition: Edition = serde_json::from_str(&std::fs::read_to_string(src).unwrap()).unwrap();
        let mut opts = RenderOptions::plain(false, true);
        opts.paper = "Sam\u{2019}s Daily".into();
        opts.city = "Los Angeles".into();
        std::fs::write(out, render_html(&edition, &opts)).unwrap();
    }

    #[test]
    fn links_are_named_not_spelled_out() {
        assert_eq!(site_name("https://www.youtube.com/watch?v=abcdefghijk"), "YouTube");
        assert_eq!(site_name("https://x.com/a/status/1"), "X");
        assert_eq!(site_name("https://www.dailywire.com/news/x"), "dailywire.com");
    }
}
