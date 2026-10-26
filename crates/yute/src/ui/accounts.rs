use crate::app::App;
use ratatui::{
    layout::Rect,
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::Paragraph,
    Frame,
};
pub fn render_accounts(f: &mut Frame, app: &App, area: Rect) {
    let auth = app.auth.lock().unwrap();
    let mode = if auth.access_token.is_some() {
        "OAuth session"
    } else if auth.sapisid.is_some() && auth.cookie_header.is_some() {
        "Cookie session"
    } else {
        "Guest — logged out"
    };
    let lines = vec![
        Line::from(Span::styled(
            app.account_name.clone(),
            Style::default()
                .fg(app.theme.text)
                .add_modifier(Modifier::BOLD),
        )),
        Line::from(app.account_handle.clone()),
        Line::from(mode),
        Line::from(""),
        Line::from(":login   Sign in with a TV device code"),
        Line::from(":logout  Sign out locally"),
        Line::from(""),
        Line::from("Account switching is not available yet."),
        Line::from("? Keyboard help    :help account"),
    ];
    f.render_widget(
        Paragraph::new(lines).style(Style::default().fg(app.theme.text_secondary)),
        area,
    );
}
