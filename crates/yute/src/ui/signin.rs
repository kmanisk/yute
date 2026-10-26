use crate::app::App;
use ratatui::{
    layout::Rect,
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Clear, Paragraph},
    Frame,
};
use yute_core::oauth::{CancellationToken, LoginEvent};

pub struct LoginModal {
    pub cancelled: CancellationToken,
    code: String,
    url: String,
    remaining: u64,
    qr: String,
}
impl LoginModal {
    pub fn new(cancelled: CancellationToken) -> Self {
        Self {
            cancelled,
            code: String::new(),
            url: String::new(),
            remaining: 0,
            qr: String::new(),
        }
    }
    pub fn update(&mut self, event: LoginEvent) {
        match event {
            LoginEvent::Code {
                user_code,
                verification_url,
                expires_in,
            } => {
                self.code = user_code;
                self.qr = qr_text(&verification_url);
                self.url = verification_url;
                self.remaining = expires_in;
            }
            LoginEvent::Remaining(seconds) => self.remaining = seconds,
        }
    }
}
pub fn qr_text(url: &str) -> String {
    let Ok(qr) = qrcode::QrCode::new(url.as_bytes()) else {
        return String::new();
    };
    let width = qr.width() as i32;
    let dark = |x: i32, y: i32| {
        x >= 0
            && y >= 0
            && x < width
            && y < width
            && qr[(x as usize, y as usize)] == qrcode::Color::Dark
    };
    let mut lines = Vec::new();
    for y in (-4..width + 4).step_by(2) {
        lines.push(
            (-4..width + 4)
                .map(|x| match (dark(x, y), dark(x, y + 1)) {
                    (true, true) => '█',
                    (true, false) => '▀',
                    (false, true) => '▄',
                    _ => ' ',
                })
                .collect::<String>(),
        );
    }
    lines.join("\n")
}
pub fn modal_area(app: &App, area: Rect) -> Option<Rect> {
    if app.login_modal.is_none() {
        return None;
    }
    let width = area.width.saturating_sub(4).min(64);
    let height = area.height.saturating_sub(2).min(29);
    Some(Rect::new(
        area.x + (area.width - width) / 2,
        area.y + (area.height - height) / 2,
        width,
        height,
    ))
}
pub fn render(f: &mut Frame, app: &App) {
    let (Some(modal), Some(area)) = (&app.login_modal, modal_area(app, f.area())) else {
        return;
    };
    f.render_widget(Clear, area);
    let block = Block::default()
        .title(" YouTube TV sign-in ")
        .borders(Borders::ALL)
        .border_style(Style::default().fg(app.theme.accent))
        .style(Style::default().bg(app.theme.bg));
    let inner = block.inner(area);
    f.render_widget(block, area);
    if modal.code.is_empty() {
        f.render_widget(Paragraph::new("Requesting code…\nEsc to cancel"), inner);
        return;
    }
    let lines = vec![
        Line::from(modal.url.clone()),
        Line::from(Span::styled(
            modal.code.clone(),
            Style::default()
                .fg(app.theme.accent)
                .add_modifier(Modifier::BOLD),
        )),
        Line::from(format!(
            "Expires in {}:{:02} · Esc cancel",
            modal.remaining / 60,
            modal.remaining % 60
        )),
    ];
    f.render_widget(
        Paragraph::new(lines),
        Rect::new(
            inner.x + 1,
            inner.y,
            inner.width.saturating_sub(2),
            3.min(inner.height),
        ),
    );
    let rows = modal.qr.lines().count() as u16;
    let cols = modal
        .qr
        .lines()
        .next()
        .map_or(0, |line| line.chars().count()) as u16;
    if rows + 4 <= inner.height && cols <= inner.width {
        f.render_widget(
            Paragraph::new(modal.qr.as_str())
                .style(Style::default().fg(Color::Black).bg(Color::White)),
            Rect::new(inner.x + (inner.width - cols) / 2, inner.y + 4, cols, rows),
        );
    } else if inner.height > 4 {
        f.render_widget(Paragraph::new("Open the address above on another device.\nEnlarge this terminal to see the QR code.").wrap(ratatui::widgets::Wrap{trim:true}),Rect::new(inner.x+1,inner.y+4,inner.width.saturating_sub(2),inner.height-4));
    }
}
#[cfg(test)]
mod tests {
    #[test]
    fn qr_has_quiet_zone_and_half_block_modules() {
        let qr = super::qr_text("https://youtube.com/activate");
        assert!(qr.lines().next().unwrap().trim().is_empty());
        assert!(qr.contains('▀') || qr.contains('▄'));
    }
}
