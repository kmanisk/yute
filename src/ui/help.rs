use crate::app::App;
use ratatui::layout::Rect;
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Clear, Paragraph};
use ratatui::Frame;

pub fn render_help(f: &mut Frame, app: &App, area: Rect) {
    let popup_w = 70u16.min(area.width.saturating_sub(4));
    let popup_h = 24u16.min(area.height.saturating_sub(2));

    let popup_x = area.x + (area.width.saturating_sub(popup_w)) / 2;
    let popup_y = area.y + (area.height.saturating_sub(popup_h)) / 2;

    let popup_rect = Rect::new(popup_x, popup_y, popup_w, popup_h);
    f.render_widget(Clear, popup_rect);

    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(app.theme.accent))
        .title(" Help - Keybindings ")
        .style(Style::default().bg(app.theme.surface));

    let inner = block.inner(popup_rect);
    f.render_widget(block, popup_rect);

    let entries = [
        ("h / j / k / l", "Move cursor across cards / items"),
        ("Enter", "Play video in external MPV window"),
        ("Shift-Enter", "Play video directly in terminal (Kitty/Sixel)"),
        ("w", "Play in external window"),
        ("o", "Open current video in browser"),
        ("/", "Focus search bar (insert mode)"),
        ("f", "Live filter loaded cards"),
        ("r / R", "Refresh / Hard refresh bypassing cache"),
        ("b", "Toggle sidebar expansion"),
        ("[ / ]", "Previous / Next filter chip"),
        ("Tab / Shift-Tab", "Cycle focus (sidebar, chips, content)"),
        ("gh / gs / gy", "Go to Home / Subscriptions / History"),
        ("gw / gp / gl", "Go to Watch Later / Playlists / Liked"),
        ("gv / gq", "Go to Local Videos / Play Queue"),
        ("Space", "Pause / Resume playback in watch mode"),
        ("i / I", "Toggle Stats for Nerds overlay in mpv"),
        ("h / l / H / L", "Seek -5s / +5s / -30s / +30s"),
        ("+ / - / M", "Volume up / Volume down / Mute"),
        ("< / >", "Speed down / Speed up (0.25x steps)"),
        ("c / d / f", "Toggle chat / description / fullscreen"),
        (":", "Command line (:q, :home, :search <q>, etc.)"),
        ("? / q", "Toggle help overlay / Go back"),
        ("Ctrl-c / Q", "Quit ytui immediately"),
    ];

    let mut lines = Vec::new();
    for (k, desc) in entries {
        lines.push(Line::from(vec![
            Span::styled(format!("  {k:<18}"), Style::default().fg(app.theme.accent).add_modifier(Modifier::BOLD)),
            Span::styled(desc, Style::default().fg(app.theme.text)),
        ]));
    }

    f.render_widget(Paragraph::new(lines), inner);
}
