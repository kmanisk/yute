use ratatui::style::Style;
use ratatui::text::Span;
use unicode_segmentation::UnicodeSegmentation;
use unicode_width::UnicodeWidthStr;

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
