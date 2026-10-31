use ratatui::buffer::CellWidth;
use ratatui::style::Style;
use ratatui::text::Span;
use std::sync::{
    atomic::{AtomicU8, Ordering},
    OnceLock, RwLock,
};
use unicode_segmentation::UnicodeSegmentation;
use unicode_width::UnicodeWidthChar;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum WidthModel {
    #[default]
    Native,
    Cluster,
    Wcwidth,
}
static MODEL: AtomicU8 = AtomicU8::new(0);
static FAILED: OnceLock<RwLock<Vec<String>>> = OnceLock::new();
static TOGGLED: OnceLock<RwLock<Option<String>>> = OnceLock::new();
pub fn set_capabilities(model: WidthModel, failed: Vec<String>) {
    MODEL.store(model as u8, Ordering::Relaxed);
    *FAILED.get_or_init(Default::default).write().unwrap() = failed;
}
pub fn model() -> WidthModel {
    match MODEL.load(Ordering::Relaxed) {
        1 => WidthModel::Cluster,
        2 => WidthModel::Wcwidth,
        _ => WidthModel::Native,
    }
}
pub fn width_with(text: &str, model: WidthModel) -> usize {
    text.graphemes(true)
        .map(|g| match model {
            WidthModel::Native => unicode_width::UnicodeWidthStr::width(g),
            WidthModel::Cluster => g
                .chars()
                .map(|c| c.width().unwrap_or(0))
                .sum::<usize>()
                .min(2),
            WidthModel::Wcwidth => g.chars().map(|c| c.width().unwrap_or(0)).sum(),
        })
        .sum()
}
pub trait DisplayWidth {
    fn width(&self) -> usize;
}
impl<T: AsRef<str> + ?Sized> DisplayWidth for T {
    fn width(&self) -> usize {
        width_with(self.as_ref(), model())
    }
}
pub fn graphemes(text: &str) -> impl Iterator<Item = &str> {
    text.graphemes(true)
}
pub fn grapheme_indices(text: &str) -> impl DoubleEndedIterator<Item = (usize, &str)> {
    text.grapheme_indices(true)
}
pub fn toggle_selected(text: &str) {
    let mut selected = TOGGLED.get_or_init(Default::default).write().unwrap();
    *selected = if selected.as_deref() == Some(text) {
        None
    } else {
        Some(text.into())
    };
}
pub fn script(c: char) -> &'static str {
    match c as u32 {
        0x0900..=0x097f => "hi",
        0x0400..=0x052f => "ru",
        0x0370..=0x03ff => "el",
        0x3040..=0x309f => "hiragana",
        0x30a0..=0x30ff => "katakana",
        0x4e00..=0x9fff => "zh",
        0xac00..=0xd7af => "ko",
        0x0e00..=0x0e7f => "th",
        0x0b80..=0x0bff => "ta",
        0x0980..=0x09ff => "bn",
        0x0600..=0x06ff => "ar",
        0x0590..=0x05ff => "he",
        0x1f000..=0x1faff | 0xfe0f => "emoji",
        _ => "latin",
    }
}
pub fn display(text: &str, policy: &str) -> String {
    let toggle = TOGGLED
        .get_or_init(Default::default)
        .read()
        .unwrap()
        .as_deref()
        == Some(text);
    let failed = FAILED.get_or_init(Default::default).read().unwrap();
    let transliterate = policy == "transliterate"
        || (policy == "auto" && text.chars().any(|c| failed.iter().any(|s| s == script(c))));
    if transliterate ^ toggle {
        return deunicode::deunicode(text);
    }
    let bidi = unicode_bidi::BidiInfo::new(text, None);
    bidi.paragraphs
        .iter()
        .map(|p| bidi.reorder_line(p, p.range.clone()))
        .collect::<Vec<_>>()
        .join("\n")
}
thread_local! { static TITLE_RECTS: std::cell::RefCell<Vec<ratatui::layout::Rect>> = const {std::cell::RefCell::new(Vec::new())}; }
pub fn begin_frame() {
    TITLE_RECTS.with(|r| r.borrow_mut().clear());
}
pub fn fill_title_rect(f: &mut ratatui::Frame, area: ratatui::layout::Rect, style: Style) {
    f.buffer_mut().set_style(area, style);
    TITLE_RECTS.with(|r| r.borrow_mut().push(area));
}
/// Paint the entire title background separately from cluster placement.
pub fn title(
    f: &mut ratatui::Frame,
    lines: Vec<String>,
    area: ratatui::layout::Rect,
    style: Style,
) {
    f.render_widget(ratatui::widgets::Block::default().style(style), area);
    for (row, line) in lines.iter().take(area.height as usize).enumerate() {
        let mut x = area.x;
        for g in graphemes(line) {
            let w = g.width().max(1).min(u16::MAX as usize) as u16;
            if x + w > area.right() {
                break;
            }
            f.buffer_mut()[(x, area.y + row as u16)]
                .set_symbol(g)
                .set_style(style)
                .set_diff_option(ratatui::buffer::CellDiffOption::ForcedWidth(
                    std::num::NonZeroU16::new(w).unwrap(),
                ));
            x += w;
        }
    }
    fill_title_rect(f, area, style);
}
/// Keep terminal shaping from moving subsequent text or borders. Only risky
/// clusters use raw anchors; ordinary text remains normal Ratatui diff output.
pub fn position_risky(buffer: &mut ratatui::buffer::Buffer) {
    use ratatui::buffer::CellDiffOption;
    TITLE_RECTS.with(|r| {
        for area in r.borrow_mut().drain(..) {
            for y in area.top()..area.bottom() {
                let cell = &mut buffer[(area.x, y)];
                let w = cell.cell_width().max(1);
                let payload = format!(
                    "\x1b[{};{}H{}\x1b[{};{}H{}\x1b[{};{}H",
                    y + 1,
                    area.x + 1,
                    " ".repeat(area.width as usize),
                    y + 1,
                    area.x + 1,
                    cell.symbol(),
                    y + 1,
                    area.x + w + 1
                );
                cell.set_symbol(&payload)
                    .set_diff_option(CellDiffOption::ForcedWidth(
                        std::num::NonZeroU16::new(w).unwrap(),
                    ));
            }
        }
    });
    let right = buffer.area.right();
    for y in buffer.area.top()..buffer.area.bottom() {
        for x in buffer.area.left()..buffer.area.right() {
            let cell = &mut buffer[(x, y)];
            let s = cell.symbol();
            if s.contains('\x1b')
                || !s.chars().any(|c| {
                    matches!(script(c), "hi" | "ta" | "bn" | "th" | "ar" | "he" | "emoji")
                        || matches!(c as u32, 0x0300..=0x036f)
                })
            {
                continue;
            }
            let w = cell.cell_width().max(1).min(right - x);
            let symbol = format!("\x1b7\x1b[{};{}H{s}\x1b8\x1b[{w}C", y + 1, x + 1);
            cell.set_symbol(&symbol)
                .set_diff_option(CellDiffOption::ForcedWidth(
                    std::num::NonZeroU16::new(w).unwrap(),
                ));
        }
    }
}

pub fn is_emoji(c: char) -> bool {
    matches!(c,
        '\u{1F600}'..='\u{1F64F}' // Emoticons
        | '\u{1F300}'..='\u{1F5FF}' // Misc Symbols and Pictographs
        | '\u{1F680}'..='\u{1F6FF}' // Transport and Map
        | '\u{1F700}'..='\u{1F77F}' // Alchemical Symbols
        | '\u{1F780}'..='\u{1F7FF}' // Geometric Shapes Extended
        | '\u{1F800}'..='\u{1F8FF}' // Supplemental Arrows-C
        | '\u{1F900}'..='\u{1F9FF}' // Supplemental Symbols and Pictographs
        | '\u{1FA00}'..='\u{1FA6F}' // Chess Symbols
        | '\u{1FA70}'..='\u{1FAFF}' // Symbols and Pictographs Extended-A
        | '\u{2600}'..='\u{26FF}'   // Misc Symbols
        | '\u{2700}'..='\u{27BF}'   // Dingbats
        | '\u{FE00}'..='\u{FE0F}'   // Variation Selectors
    )
}

pub fn strip_emojis(s: &str) -> String {
    s.chars().filter(|c| !is_emoji(*c)).collect()
}

pub fn sanitize_title(title: &str, strip_emoji: bool) -> String {
    let mut base = if strip_emoji {
        strip_emojis(title)
    } else {
        title.to_string()
    };

    // Collapse whitespace
    base = base.split_whitespace().collect::<Vec<_>>().join(" ");

    // Check for trailing run of 2+ hashtags (e.g. "Title #shorts #gaming")
    let words: Vec<&str> = base.split_whitespace().collect();
    let mut trailing_hash_count = 0;
    for w in words.iter().rev() {
        if w.starts_with('#') {
            trailing_hash_count += 1;
        } else {
            break;
        }
    }

    if trailing_hash_count >= 2 {
        let keep_len = words.len().saturating_sub(trailing_hash_count);
        base = words[..keep_len].join(" ");
    }

    // Strip trailing '|', '-', '—', '\', ':' and spaces
    let mut trimmed = base.trim();
    loop {
        let before = trimmed;
        trimmed = trimmed
            .trim_end_matches('|')
            .trim_end_matches('-')
            .trim_end_matches('—')
            .trim_end_matches('\\')
            .trim_end_matches(':')
            .trim();
        if trimmed == before {
            break;
        }
    }

    trimmed.to_string()
}

pub fn complex_width(text: &str, width: usize) -> usize {
    if text.chars().any(|c| matches!(c as u32, 0x0900..=0x0dff)) {
        width.saturating_sub(1)
    } else {
        width
    }
}
pub fn title_for(title: &str, config: &crate::config::Config) -> String {
    let policy = if config.strip_emoji() && config.text.emoji == "auto" {
        "strip"
    } else {
        config.text.emoji.as_str()
    };
    let title = display(title, &config.text.scripts);
    let mut safe = String::new();
    for g in title.graphemes(true) {
        let emoji =
            g.chars().any(|c| matches!(c as u32, 0x1f000..=0x1faff)) || g.contains('\u{fe0f}');
        match policy {
            "strip" if emoji => (),
            "auto"
                if emoji
                    && (g.contains('\u{200d}')
                        || g.contains('\u{fe0f}')
                        || g.chars().any(|c| matches!(c as u32, 0x1f1e6..=0x1f1ff))) =>
            {
                safe.push('*')
            }
            _ => safe.push_str(g),
        }
    }
    sanitize_title(&safe, false)
}
pub fn wrap_and_truncate_title(
    clean_title: &str,
    max_width: usize,
    max_lines: usize,
) -> Vec<String> {
    let max_width = complex_width(clean_title, max_width);
    if max_width == 0 || max_lines == 0 {
        return vec![];
    }
    let text = clean_title.split_whitespace().collect::<Vec<_>>().join(" ");
    let mut remaining = text.as_str();
    let mut lines = vec![];
    while !remaining.is_empty() && lines.len() < max_lines {
        if remaining.width() <= max_width {
            lines.push(remaining.into());
            break;
        }
        if lines.len() + 1 == max_lines {
            lines.push(truncate_str(remaining, max_width));
            break;
        }
        let mut width = 0;
        let mut end = 0;
        for (offset, g) in remaining.grapheme_indices(true) {
            if width + g.width() > max_width {
                break;
            }
            width += g.width();
            end = offset + g.len();
        }
        if end == 0 {
            lines.push("…".into());
            break;
        }
        let split = remaining[..end]
            .rfind(' ')
            .filter(|i| *i > 0)
            .unwrap_or(end);
        lines.push(remaining[..split].trim_end().into());
        remaining = remaining[split..].trim_start();
    }
    lines
}

/// Preserve YouTube's localized/abbreviated text; compact plain numeric counts.
pub fn compact_views(input: &str) -> String {
    let input = input.trim();
    let number = input
        .strip_suffix(" views")
        .or_else(|| input.strip_suffix(" view"))
        .unwrap_or(input);
    if !number.chars().all(|c| c.is_ascii_digit() || c == ',') {
        return input.to_string();
    }
    let Ok(count) = number.replace(',', "").parse::<u64>() else {
        return input.to_string();
    };
    let (divisor, suffix) = if count >= 1_000_000_000 {
        (1_000_000_000.0, "B")
    } else if count >= 1_000_000 {
        (1_000_000.0, "M")
    } else if count >= 1_000 {
        (1_000.0, "K")
    } else {
        return format!("{count} views");
    };
    let value = format!("{:.1}", count as f64 / divisor);
    format!("{}{suffix} views", value.trim_end_matches(".0"))
}

pub fn format_meta_line(
    channel: &str,
    verified: bool,
    views: Option<&str>,
    published: Option<&str>,
    max_width: usize,
    secondary_style: Style,
) -> Vec<Span<'static>> {
    let clean_channel = if channel == "YouTube Shorts" || channel.eq_ignore_ascii_case("shorts") {
        ""
    } else {
        channel.trim()
    };

    let v_mark = if verified { " [v]" } else { "" };
    let compact = views.map(compact_views);
    let views_str = compact
        .as_deref()
        .map(|v| v.trim())
        .filter(|v| !v.is_empty());
    let pub_str = published
        .map(|p| p.trim())
        .filter(|p| !p.is_empty() && !p.chars().all(|c| matches!(c, '•' | '·' | ' ' | '|')));

    // Priority:
    // 1. All fields if fit
    // 2. Truncate channel with ellipsis (if channel can have >= 6 chars)
    // 3. Drop published (age)
    // 4. Drop views
    // 5. Truncate channel only

    let build_spans = |chan: &str, has_views: bool, has_pub: bool| -> Vec<Span<'static>> {
        let mut spans = Vec::new();
        if !chan.is_empty() {
            spans.push(Span::styled(format!("{chan}{v_mark}"), secondary_style));
        }
        if has_views {
            if let Some(vw) = views_str {
                if !spans.is_empty() {
                    spans.push(Span::styled(" · ", secondary_style));
                }
                spans.push(Span::styled(vw.to_string(), secondary_style));
            }
        }
        if has_pub {
            if let Some(pb) = pub_str {
                if !spans.is_empty() {
                    spans.push(Span::styled(" · ", secondary_style));
                }
                spans.push(Span::styled(pb.to_string(), secondary_style));
            }
        }
        spans
    };

    let calc_width = |chan: &str, has_views: bool, has_pub: bool| -> usize {
        let mut w = 0;
        if !chan.is_empty() {
            w += chan.width() + v_mark.width();
        }
        if has_views {
            if let Some(vw) = views_str {
                if w > 0 {
                    w += 3;
                } // " · "
                w += vw.width();
            }
        }
        if has_pub {
            if let Some(pb) = pub_str {
                if w > 0 {
                    w += 3;
                } // " · "
                w += pb.width();
            }
        }
        w
    };

    // Try full line with views & published
    let full_w = calc_width(clean_channel, views_str.is_some(), pub_str.is_some());
    if full_w <= max_width {
        return build_spans(clean_channel, views_str.is_some(), pub_str.is_some());
    }

    // Try truncating channel while keeping views & published (only if views or pub exist)
    if (views_str.is_some() || pub_str.is_some()) && !clean_channel.is_empty() {
        let fixed_views_pub = calc_width("", views_str.is_some(), pub_str.is_some());
        let avail_for_chan = max_width.saturating_sub(fixed_views_pub + 3 + v_mark.width());
        if avail_for_chan >= 6 {
            let truncated_chan = truncate_str(clean_channel, avail_for_chan);
            return build_spans(&truncated_chan, views_str.is_some(), pub_str.is_some());
        }
    }

    // Drop published (age), keep views
    if views_str.is_some() {
        let w_views = calc_width(clean_channel, true, false);
        if w_views <= max_width {
            return build_spans(clean_channel, true, false);
        }
        let fixed_views = calc_width("", true, false);
        let avail_chan = max_width.saturating_sub(fixed_views + 3 + v_mark.width());
        if avail_chan >= 6 && !clean_channel.is_empty() {
            let truncated_chan = truncate_str(clean_channel, avail_chan);
            return build_spans(&truncated_chan, true, false);
        }
    }

    // Drop views, keep only channel
    if !clean_channel.is_empty() {
        let chan_w = clean_channel.width() + v_mark.width();
        if chan_w <= max_width {
            return build_spans(clean_channel, false, false);
        }
        let avail_chan = max_width.saturating_sub(v_mark.width());
        let truncated_chan = truncate_str(clean_channel, avail_chan);
        return build_spans(&truncated_chan, false, false);
    }

    // If channel was empty (e.g. Shorts), show views · age or views alone
    if views_str.is_some() && pub_str.is_some() {
        let w = calc_width("", true, true);
        if w <= max_width {
            return build_spans("", true, true);
        }
    }
    if views_str.is_some() {
        return build_spans("", true, false);
    }

    Vec::new()
}

pub fn truncate_str(s: &str, max_width: usize) -> String {
    if s.width() <= max_width {
        return s.to_string();
    }
    if max_width == 0 {
        return String::new();
    }
    if max_width == 1 {
        return "…".to_string();
    }

    let target = max_width.saturating_sub(1);
    let mut res = String::new();
    let mut current_w = 0;
    for ch in s.graphemes(true) {
        let ch_w = ch.width();
        if current_w + ch_w <= target {
            res.push_str(ch);
            current_w += ch_w;
        } else {
            break;
        }
    }
    while res.chars().last().is_some_and(char::is_whitespace) {
        res.pop();
    }
    res.push('…');
    res
}

#[cfg(test)]
mod count_tests {
    #[test]
    fn plain_counts_are_compact_but_youtube_text_is_preserved() {
        for (input, expected) in [
            ("78000 views", "78K views"),
            ("10000000 views", "10M views"),
            ("142000 views", "142K views"),
            ("1,200,000 views", "1.2M views"),
            ("378 views", "378 views"),
            ("3.5M views", "3.5M views"),
            ("No views", "No views"),
        ] {
            assert_eq!(super::compact_views(input), expected);
        }
        let spans = super::format_meta_line(
            "",
            false,
            Some("78000 views"),
            Some("2 days ago"),
            40,
            ratatui::style::Style::default(),
        );
        assert_eq!(
            spans.iter().map(|s| s.content.as_ref()).collect::<String>(),
            "78K views · 2 days ago"
        );
    }
}

/// Lossless wrapping for bodies and diagnostics; no whitespace normalization.
pub fn wrap_lossless(text: &str, width: usize) -> Vec<&str> {
    if text.is_empty() {
        return vec![];
    }
    let mut lines = vec![];
    let mut start = 0;
    let mut used = 0;
    for (i, g) in grapheme_indices(text) {
        let w = g.width();
        if i > start && used + w > width {
            lines.push(&text[start..i]);
            start = i;
            used = 0;
        }
        used += w;
    }
    lines.push(&text[start..]);
    lines
}
#[cfg(test)]
mod terminal_tests {
    use super::*;
    const TEXTS: &[&str] = &[
        "रजनीकांत को पता चल गया इंडिया में 20 लाख करोड़ का काला धन है",
        "Привет, мир — как дела?",
        "日本語のタイトル ひらがな カタカナ",
        "안녕하세요",
        "中文标题",
        "ภาษาไทย",
        "தமிழ்",
        "বাংলা",
        "مرحبا بالعالم",
        "שלום עולם",
        "naïve cafe\u{301}",
        "\u{1f1ee}\u{1f1f3}\u{1f469}\u{200d}\u{1f4bb}\u{1f44d}\u{1f3fd}\u{2764}\u{fe0f}",
    ];
    #[test]
    fn wrapping_keeps_all_graphemes_and_ellipsis_fits() {
        for s in TEXTS {
            for w in [4, 8, 18, 30] {
                let lines = wrap_lossless(s, w);
                assert_eq!(lines.concat(), *s);
                for line in &lines {
                    assert!(line.width() <= w);
                }
                assert!(truncate_str(s, w).width() <= w);
                for line in wrap_and_truncate_title(s, w, 2) {
                    assert!(line.width() <= w);
                }
            }
        }
    }
    #[test]
    fn selected_rect_has_no_unstyled_continuation_cells() {
        use ratatui::{backend::TestBackend, layout::Rect, style::Modifier, Terminal};
        for s in TEXTS {
            let mut t = Terminal::new(TestBackend::new(48, 6)).unwrap();
            let style = Style::default().add_modifier(Modifier::REVERSED);
            let frame = t
                .draw(|f| {
                    title(
                        f,
                        wrap_and_truncate_title(s, 40, 2),
                        Rect::new(2, 2, 40, 2),
                        style,
                    )
                })
                .unwrap();
            for y in 2..4 {
                for x in 2..42 {
                    assert!(
                        frame.buffer[(x, y)].modifier.contains(Modifier::REVERSED),
                        "{s}: {x},{y}"
                    );
                }
            }
        }
    }
    #[test]
    fn text_width_is_only_owned_here() {
        fn scan(p: &std::path::Path) {
            for e in std::fs::read_dir(p).unwrap() {
                let p = e.unwrap().path();
                if p.is_dir() {
                    scan(&p);
                } else if p.extension().is_some_and(|e| e == "rs")
                    && p.file_name().unwrap() != "text.rs"
                {
                    let s = std::fs::read_to_string(&p).unwrap();
                    assert!(!s.contains("unicode_width::"), "{}", p.display());
                }
            }
        }
        scan(&std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src"));
    }
    #[test]
    fn explicit_cluster_position_keeps_following_border_intact() {
        let mut b = ratatui::buffer::Buffer::empty(ratatui::layout::Rect::new(0, 0, 20, 2));
        b.set_string(1, 0, "क्ष", Style::default());
        b[(19, 0)].set_symbol("│");
        position_risky(&mut b);
        assert!(b[(1, 0)].symbol().contains("\x1b[1;2H"));
        assert_eq!(b[(19, 0)].symbol(), "│");
    }
}
