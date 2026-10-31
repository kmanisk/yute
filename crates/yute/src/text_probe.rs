//! Bounded terminal probes use the existing input owner. No key bytes are logged.
use crate::ui::text::{self, WidthModel};
use serde::{Deserialize, Serialize};
use std::{
    io::{IsTerminal, Write},
    sync::{
        atomic::{AtomicBool, Ordering},
        OnceLock,
    },
    time::Duration,
};
use tokio::sync::broadcast;
#[derive(Clone, Debug)]
pub enum Reply {
    Cursor(u16, u16),
    Mode(u8),
    Version(String),
}
fn replies() -> &'static broadcast::Sender<Reply> {
    static TX: OnceLock<broadcast::Sender<Reply>> = OnceLock::new();
    TX.get_or_init(|| broadcast::channel(128).0)
}
pub fn reply(r: Reply) {
    let _ = replies().send(r);
}
static RESET: AtomicBool = AtomicBool::new(false);
pub fn restore() {
    if RESET.swap(false, Ordering::Relaxed) {
        let _ = std::io::stdout().write_all(b"\x1b[?2027l");
    }
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Sample {
    pub script: String,
    pub text: String,
    pub expected: usize,
    pub measured: Option<usize>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Probe {
    pub terminal: String,
    pub version: String,
    pub width_model: WidthModel,
    pub mode_2027: Option<u8>,
    pub samples: Vec<Sample>,
}
pub const SAMPLES: &[(&str, &str)] = &[
    ("hi", "कि की कृ"),
    ("hi", "क्ष त्र ज्ञ"),
    ("hi", "क़ फ़ ड़"),
    ("ru", "Привет, мир — как дела?"),
    ("el", "Καλημέρα"),
    ("hiragana", "ひらがな"),
    ("katakana", "カタカナ"),
    ("zh", "日本語のタイトル"),
    ("ko", "안녕하세요"),
    ("zh", "中文标题"),
    ("th", "ภาษาไทย"),
    ("ta", "தமிழ்"),
    ("bn", "বাংলা"),
    ("ar", "مرحبا بالعالم"),
    ("he", "שלום עולם"),
    ("latin", "naïve cafe\u{301}"),
    ("emoji", "\u{1f1ee}\u{1f1f3}"),
    ("emoji", "\u{1f469}\u{200d}\u{1f4bb}"),
    ("emoji", "\u{1f44d}\u{1f3fd}"),
    ("emoji", "\u{2764}\u{fe0f}"),
];
fn identity() -> String {
    ["TERM", "TERM_PROGRAM", "TERM_PROGRAM_VERSION", "COLORTERM"]
        .map(|k| std::env::var(k).unwrap_or_default())
        .join("|")
}
fn path(key: &str) -> std::path::PathBuf {
    use std::hash::{Hash, Hasher};
    let mut h = std::collections::hash_map::DefaultHasher::new();
    key.hash(&mut h);
    crate::config::cache_dir().join(format!("text-probe-v1-{:x}.json", h.finish()))
}
impl Probe {
    pub fn failed(&self) -> Vec<String> {
        let mut v = vec![];
        for s in &self.samples {
            if s.measured.is_some_and(|w| w != s.expected) && !v.contains(&s.script) {
                v.push(s.script.clone());
            }
        }
        v
    }
    pub fn install(&self) {
        text::set_capabilities(self.width_model, self.failed());
    }
    pub fn summary(&self) -> String {
        format!(
            "terminal={} version={} width={:?} mode2027={:?}; measured {}/{}; mismatches={:?}",
            self.terminal,
            self.version,
            self.width_model,
            self.mode_2027,
            self.samples.iter().filter(|s| s.measured.is_some()).count(),
            self.samples.len(),
            self.failed()
        )
    }
}
async fn receive(rx: &mut broadcast::Receiver<Reply>) -> Option<Reply> {
    tokio::time::timeout(Duration::from_millis(35), rx.recv())
        .await
        .ok()?
        .ok()
}
pub async fn run(cached: bool) -> Probe {
    let terminal = identity();
    let mut rx = replies().subscribe();
    let mut out = std::io::stdout();
    let mut p = Probe {
        terminal,
        version: "unavailable".into(),
        width_model: WidthModel::Native,
        mode_2027: None,
        samples: vec![],
    };
    if !std::io::stdin().is_terminal() || !out.is_terminal() {
        return p;
    }
    let _ = out.write_all(b"\x1b[>0c\x1b[?2027$p");
    let _ = out.flush();
    for _ in 0..3 {
        match receive(&mut rx).await {
            Some(Reply::Version(v)) => p.version = v,
            Some(Reply::Mode(m)) => p.mode_2027 = Some(m),
            _ => break,
        }
    }
    let key = format!("{}|{}|{:?}", p.terminal, p.version, p.mode_2027);
    if p.mode_2027 == Some(2) {
        let _ = out.write_all(b"\x1b[?2027h");
        let _ = out.flush();
        RESET.store(true, Ordering::Relaxed);
    }
    if cached && p.version != "unavailable" {
        if let Ok(bytes) = tokio::fs::read(path(&key)).await {
            if let Ok(old) = serde_json::from_slice::<Probe>(&bytes) {
                if old.mode_2027 == p.mode_2027 {
                    old.install();
                    return old;
                }
            }
        }
    }
    let row = crossterm::terminal::size().map(|(_, h)| h).unwrap_or(1);
    for &(script, s) in SAMPLES {
        // Reserve the bottom status row; normal keys stay with the input owner.
        let command = format!("\x1b[?2026h\x1b[{row};1H\x1b[2K{s}\x1b[6n");
        let _ = out.write_all(command.as_bytes());
        let _ = out.flush();
        let measured = match receive(&mut rx).await {
            Some(Reply::Cursor(r, c)) if r == row => Some(c.saturating_sub(1) as usize),
            _ => None,
        };
        let _ = out.write_all(format!("\x1b[{row};1H\x1b[2K\x1b[1;1H\x1b[?2026l").as_bytes());
        let _ = out.flush();
        p.samples.push(Sample {
            script: script.into(),
            text: s.into(),
            expected: 0,
            measured,
        });
        if measured.is_none() {
            break;
        }
    }
    p.width_model = [WidthModel::Native, WidthModel::Cluster, WidthModel::Wcwidth]
        .into_iter()
        .min_by_key(|m| {
            p.samples
                .iter()
                .filter(|s| {
                    s.measured
                        .is_some_and(|w| w != text::width_with(&s.text, *m))
                })
                .count()
        })
        .unwrap_or_default();
    for s in &mut p.samples {
        s.expected = text::width_with(&s.text, p.width_model);
    }
    // Only retain an enabled mode if the measured cluster model actually fits.
    if !p.samples.iter().any(|s| s.measured.is_some()) {
        restore();
    }
    p.install();
    if p.samples.iter().any(|s| s.measured.is_some()) {
        let _ = tokio::fs::write(path(&key), serde_json::to_vec(&p).unwrap_or_default()).await;
    }
    p
}
pub async fn font_coverage() -> Vec<String> {
    let mut result = vec![];
    for lang in ["hi", "ja", "ko", "zh", "ar", "th", "ta", "bn", "emoji"] {
        let pattern = if lang == "emoji" {
            ":charset=1f469".into()
        } else {
            format!(":lang={lang}")
        };
        let mut cmd = tokio::process::Command::new("fc-list");
        cmd.args(["-f", "%{family}\n", &pattern]).kill_on_drop(true);
        let covered = tokio::time::timeout(Duration::from_millis(300), cmd.output())
            .await
            .ok()
            .and_then(Result::ok)
            .is_some_and(|o| o.status.success() && !o.stdout.is_empty());
        result.push(format!(
            "font {lang}: {}",
            if covered {
                "available"
            } else if matches!(lang, "ja" | "ko" | "zh") {
                "missing/unknown; Arch: noto-fonts-cjk"
            } else if lang == "emoji" {
                "missing/unknown; Arch: noto-fonts-emoji"
            } else {
                "missing/unknown; Arch: noto-fonts"
            }
        ));
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;
    #[tokio::test]
    async fn terminal_replies_are_not_keyboard_events() {
        let mut rx = replies().subscribe();
        let mut d = crate::terminal_input::Decoder::default();
        let keys = d.feed(b"\x1b[?2027;1$y\x1b[31;9R\x1b[>1;012800;0cj");
        assert!(matches!(rx.recv().await.unwrap(), Reply::Mode(1)));
        assert!(matches!(rx.recv().await.unwrap(), Reply::Cursor(31, 9)));
        assert!(matches!(rx.recv().await.unwrap(), Reply::Version(_)));
        assert_eq!(keys.len(), 1);
        assert!(matches!(
            keys[0],
            crossterm::event::Event::Key(crossterm::event::KeyEvent {
                code: crossterm::event::KeyCode::Char('j'),
                ..
            })
        ));
    }
    #[test]
    fn missing_cpr_does_not_misclassify_scripts() {
        let p = Probe {
            terminal: "fixture".into(),
            version: "unavailable".into(),
            width_model: WidthModel::Native,
            mode_2027: None,
            samples: vec![Sample {
                script: "hi".into(),
                text: "कि".into(),
                expected: 2,
                measured: None,
            }],
        };
        assert!(p.failed().is_empty());
    }
}
