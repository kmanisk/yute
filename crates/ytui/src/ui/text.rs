use ratatui::style::Style;
use ratatui::text::Span;
use unicode_width::{UnicodeWidthChar, UnicodeWidthStr};

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

pub fn wrap_and_truncate_title(clean_title: &str, max_width: usize, max_lines: usize) -> Vec<String> {
    if max_width == 0 || max_lines == 0 || clean_title.is_empty() {
        return Vec::new();
    }

    let words: Vec<&str> = clean_title.split_whitespace().collect();
    if words.is_empty() {
        return Vec::new();
    }

    let mut lines = Vec::new();
    let mut current_line = String::new();
    let mut current_width = 0;
    let mut word_idx = 0;

    while word_idx < words.len() {
        let word = words[word_idx];
        let word_w = word.width();

        let space_w = if current_line.is_empty() { 0 } else { 1 };

        if current_width + space_w + word_w <= max_width {
            if !current_line.is_empty() {
                current_line.push(' ');
                current_width += 1;
            }
            current_line.push_str(word);
            current_width += word_w;
            word_idx += 1;
        } else if lines.len() + 1 < max_lines {
            // Move to next line
            if current_line.is_empty() {
                // Word alone is wider than max_width, truncate it
                let mut fit_word = String::new();
                let mut fit_w = 0;
                for ch in word.chars() {
                    let ch_w = ch.width().unwrap_or(1);
                    if fit_w + ch_w <= max_width {
                        fit_word.push(ch);
                        fit_w += ch_w;
                    } else {
                        break;
                    }
                }
                lines.push(fit_word);
                word_idx += 1;
            } else {
                lines.push(current_line);
                current_line = String::new();
                current_width = 0;
            }
        } else {
            // We are on the last allowed line and cannot fit the next word
            // Check if we need to add ellipsis
            let ellipsis_w = 1;

            if current_width + ellipsis_w <= max_width {
                current_line.push('…');
            } else {
                // Back up words until ellipsis fits
                let line_words: Vec<&str> = current_line.split_whitespace().collect();
                let mut rebuilt = String::new();
                let mut r_width = 0;
                for lw in line_words {
                    let lw_w = lw.width();
                    let s_w = if rebuilt.is_empty() { 0 } else { 1 };
                    if r_width + s_w + lw_w + ellipsis_w <= max_width {
                        if !rebuilt.is_empty() {
                            rebuilt.push(' ');
                            r_width += 1;
                        }
                        rebuilt.push_str(lw);
                        r_width += lw_w;
                    } else {
                        break;
                    }
                }
                rebuilt.push('…');
                current_line = rebuilt;
            }
            break;
        }
    }

    if !current_line.is_empty() && lines.len() < max_lines {
        lines.push(current_line);
    }

    lines
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
    let views_str = views.map(|v| v.trim()).filter(|v| !v.is_empty());
    let pub_str = published.map(|p| p.trim()).filter(|p| !p.is_empty());

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
                if w > 0 { w += 3; } // " · "
                w += vw.width();
            }
        }
        if has_pub {
            if let Some(pb) = pub_str {
                if w > 0 { w += 3; } // " · "
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
    if max_width <= 1 {
        return "…".to_string();
    }

    let target = max_width.saturating_sub(1);
    let mut res = String::new();
    let mut current_w = 0;
    for ch in s.chars() {
        let ch_w = ch.width().unwrap_or(1);
        if current_w + ch_w <= target {
            res.push(ch);
            current_w += ch_w;
        } else {
            break;
        }
    }
    res.push('…');
    res
}
