use crate::app::{App, Screen};
use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::Paragraph;
use ratatui::Frame;

pub fn render_statusbar(f: &mut Frame, app: &App, area: Rect) {
    if area.height == 0 || area.width == 0 {
        return;
    }

    let rows = if area.height >= 2 {
        Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Length(1), Constraint::Length(1)])
            .split(area)
    } else {
        Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Length(1)])
            .split(area)
    };

    // Row 0: Context-sensitive key hints
    let hints = match app.screen {
        Screen::Watch => "Space pause · h/l seek · +/- vol · </> speed · i stats · c chat · d desc · f full · q back",
        _ => "j/k move · Enter play · Shift+Enter terminal · / search · f filter · r refresh · b sidebar · ? help · q back",
    };

    let p_hints = Paragraph::new(Line::from(vec![
        Span::styled(format!(" {hints}"), Style::default().fg(app.theme.text_secondary)),
    ]));
    f.render_widget(p_hints, rows[0]);

    if rows.len() > 1 {
        // Row 1: status box line with [backend] [status_text]
        let backend_str = "innertube";

        let status_spans = vec![
            Span::styled(format!(" [{backend_str}]"), Style::default().fg(app.theme.text_secondary)),
            Span::raw("  "),
            Span::styled(
                format!("[ {} ]", app.status_text),
                Style::default()
                    .fg(app.theme.accent)
                    .add_modifier(Modifier::BOLD),
            ),
        ];

        let p_status = Paragraph::new(Line::from(status_spans));
        f.render_widget(p_status, rows[1]);
    }
}
