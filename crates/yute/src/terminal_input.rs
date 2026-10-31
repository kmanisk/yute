//! One stdin owner demultiplexes keys and terminal replies. No competing queries.
use crossterm::event::{Event, KeyCode, KeyEvent, KeyModifiers};
use std::os::fd::{AsRawFd, FromRawFd, OwnedFd};
use tokio::sync::mpsc::{channel, Receiver, Sender};
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Report {
    pub kitty: bool,
    pub sixel: bool,
    pub iterm2: bool,
    pub light: Option<bool>,
    pub cells: Option<(u16, u16)>,
    pub background: Option<(u8, u8, u8)>,
}
#[derive(Default)]
pub struct Decoder {
    buffer: Vec<u8>,
    pub report: Report,
}
fn mods(n: u32) -> KeyModifiers {
    let n = n.saturating_sub(1);
    let mut m = KeyModifiers::empty();
    if n & 1 != 0 {
        m |= KeyModifiers::SHIFT;
    }
    if n & 2 != 0 {
        m |= KeyModifiers::ALT;
    }
    if n & 4 != 0 {
        m |= KeyModifiers::CONTROL;
    }
    m
}
impl Decoder {
    pub fn pending(&self) -> bool {
        !self.buffer.is_empty()
    }
    pub fn escape(&mut self) -> Option<Event> {
        if self.buffer == [27] {
            self.buffer.clear();
            Some(Event::Key(KeyEvent::new(
                KeyCode::Esc,
                KeyModifiers::empty(),
            )))
        } else {
            None
        }
    }
    pub fn feed(&mut self, bytes: &[u8]) -> Vec<Event> {
        self.buffer.extend_from_slice(bytes);
        if self.buffer.len() > 16384 {
            self.buffer.clear();
        }
        let mut events = vec![];
        loop {
            if self.buffer.is_empty() {
                break;
            }
            if self.buffer.starts_with(b"\x1b[200~") {
                if let Some(end) = self.buffer.windows(6).position(|p| p == b"\x1b[201~") {
                    let text = String::from_utf8_lossy(&self.buffer[6..end]).into_owned();
                    events.push(Event::Paste(text));
                    self.buffer.drain(..end + 6);
                    continue;
                } else {
                    break;
                }
            }
            let b = self.buffer[0];
            if b == 27 {
                if self.buffer.len() < 2 {
                    break;
                }
                if matches!(self.buffer[1], b']' | b'_' | b'P') {
                    let end = self
                        .buffer
                        .windows(2)
                        .position(|p| p == b"\x1b\\")
                        .map(|p| p + 2)
                        .or_else(|| self.buffer.iter().position(|b| *b == 7).map(|p| p + 1));
                    let Some(end) = end else {
                        break;
                    };
                    let s = String::from_utf8_lossy(&self.buffer[..end]).to_string();
                    if s.starts_with("\x1b_Gi=31;") && s.contains(";OK") {
                        self.report.kitty = true;
                    }
                    if s.contains("1337;ReportCellSize=") {
                        self.report.iterm2 = true;
                        if let Some(values) = s.split("ReportCellSize=").nth(1) {
                            let nums = values
                                .trim_end_matches(['\x1b', '\\', '\x07'])
                                .split(';')
                                .filter_map(|v| v.parse::<f64>().ok())
                                .collect::<Vec<_>>();
                            if nums.len() >= 2 {
                                let scale = nums.get(2).copied().unwrap_or(1.);
                                let (w, h) = (nums[1] * scale, nums[0] * scale);
                                if w >= 1. && h >= 1. && w <= 1024. && h <= 1024. {
                                    self.report.cells = Some((w.round() as u16, h.round() as u16));
                                }
                            }
                        }
                    }
                    if let Some(rgb) = s.strip_prefix("\x1b]11;rgb:") {
                        let values = rgb
                            .trim_end_matches(['\x1b', '\\', '\x07'])
                            .split('/')
                            .map(|v| {
                                if !(1..=4).contains(&v.len()) {
                                    return None;
                                }
                                u32::from_str_radix(v, 16)
                                    .ok()
                                    .filter(|_| (1..=4).contains(&v.len()))
                                    .map(|n| {
                                        ((n * 255) / ((1u32 << (4 * v.len().min(4))) - 1)) as u8
                                    })
                            })
                            .collect::<Option<Vec<_>>>();
                        if let Some(v) = values.filter(|v| v.len() == 3) {
                            self.report.background = Some((v[0], v[1], v[2]));
                            self.report.light = Some(
                                u32::from(v[0]) * 299
                                    + u32::from(v[1]) * 587
                                    + u32::from(v[2]) * 114
                                    > 128000,
                            );
                        }
                    }
                    self.buffer.drain(..end);
                    continue;
                }
                if self.buffer[1] == b'[' || self.buffer[1] == b'O' {
                    let Some(end) = self
                        .buffer
                        .iter()
                        .enumerate()
                        .skip(2)
                        .find(|(_, b)| **b >= 0x40 && **b <= 0x7e)
                        .map(|(i, _)| i)
                    else {
                        break;
                    };
                    let final_byte = self.buffer[end];
                    let args = String::from_utf8_lossy(&self.buffer[2..end]);
                    let nums = args
                        .trim_start_matches('?')
                        .split(';')
                        .map(|v| {
                            v.split(':')
                                .next()
                                .unwrap_or("")
                                .parse::<u32>()
                                .unwrap_or(1)
                        })
                        .collect::<Vec<_>>();
                    if final_byte == b'R' && nums.len() == 2 && !args.starts_with('?') {
                        crate::text_probe::reply(crate::text_probe::Reply::Cursor(
                            nums[0].min(u16::MAX as u32) as u16,
                            nums[1].min(u16::MAX as u32) as u16,
                        ));
                    }
                    if final_byte == b'y' && args.starts_with("?2027;") && args.ends_with('$') {
                        if let Some(m) = args
                            .trim_end_matches('$')
                            .split(';')
                            .nth(1)
                            .and_then(|s| s.parse::<u8>().ok())
                        {
                            crate::text_probe::reply(crate::text_probe::Reply::Mode(m));
                        }
                    }
                    if final_byte == b'c' && args.starts_with('>') {
                        crate::text_probe::reply(crate::text_probe::Reply::Version(
                            args.to_string(),
                        ));
                    }
                    let modifier = mods(*nums.get(1).unwrap_or(&1));
                    let mut code = match final_byte {
                        b'A' => Some(KeyCode::Up),
                        b'B' => Some(KeyCode::Down),
                        b'C' => Some(KeyCode::Right),
                        b'D' => Some(KeyCode::Left),
                        b'H' => Some(KeyCode::Home),
                        b'F' => Some(KeyCode::End),
                        b'Z' => Some(KeyCode::BackTab),
                        b'~' => match nums[0] {
                            1 | 7 => Some(KeyCode::Home),
                            4 | 8 => Some(KeyCode::End),
                            3 => Some(KeyCode::Delete),
                            5 => Some(KeyCode::PageUp),
                            6 => Some(KeyCode::PageDown),
                            _ => None,
                        },
                        b'u' => match nums[0] {
                            13 => Some(KeyCode::Enter),
                            27 => Some(KeyCode::Esc),
                            9 => Some(KeyCode::Tab),
                            127 => Some(KeyCode::Backspace),
                            n => char::from_u32(n).map(KeyCode::Char),
                        },
                        _ => None,
                    };
                    if final_byte == b'n' && nums.first() == Some(&997) {
                        self.report.light = match nums.get(1) {
                            Some(1) => Some(false),
                            Some(2) => Some(true),
                            _ => self.report.light,
                        };
                    }
                    if final_byte == b'c' && args.starts_with('?') {
                        self.report.sixel = nums.contains(&4);
                    }
                    if final_byte == b't'
                        && nums.len() == 3
                        && nums[0] == 6
                        && (1..=1024).contains(&nums[1])
                        && (1..=1024).contains(&nums[2])
                    {
                        self.report.cells = Some((nums[2] as u16, nums[1] as u16));
                    }
                    if final_byte == b'I' {
                        events.push(Event::FocusGained);
                    }
                    if final_byte == b'O' {
                        events.push(Event::FocusLost);
                    }
                    if final_byte == b'u' && modifier.contains(KeyModifiers::SHIFT) {
                        if let Some(n) = args
                            .split(';')
                            .next()
                            .and_then(|v| v.split(':').nth(1))
                            .and_then(|s| s.parse::<u32>().ok())
                            .and_then(char::from_u32)
                        {
                            code = Some(KeyCode::Char(n));
                        } else if let Some(KeyCode::Char(c)) = code {
                            if c.is_ascii_lowercase() {
                                code = Some(KeyCode::Char(c.to_ascii_uppercase()));
                            }
                        }
                    }
                    if args.split(';').nth(1).is_some_and(|s| s.ends_with(":3")) {
                        code = None;
                    }
                    if args.starts_with('?') {
                        code = None;
                    }
                    if let Some(code) = code {
                        events.push(Event::Key(KeyEvent::new(
                            code,
                            if final_byte == b'Z' {
                                KeyModifiers::SHIFT
                            } else {
                                modifier
                            },
                        )));
                    }
                    self.buffer.drain(..=end);
                    continue;
                }
                let next = self.buffer[1];
                if next.is_ascii() {
                    events.push(Event::Key(KeyEvent::new(
                        KeyCode::Char(next as char),
                        KeyModifiers::ALT,
                    )));
                    self.buffer.drain(..2);
                    continue;
                }
                self.buffer.remove(0);
                continue;
            }
            if b < 32 || b == 127 {
                let (code, m) = match b {
                    13 | 10 => (KeyCode::Enter, KeyModifiers::empty()),
                    9 => (KeyCode::Tab, KeyModifiers::empty()),
                    127 | 8 => (KeyCode::Backspace, KeyModifiers::empty()),
                    1..=26 => (KeyCode::Char((b + b'a' - 1) as char), KeyModifiers::CONTROL),
                    _ => (KeyCode::Null, KeyModifiers::empty()),
                };
                events.push(Event::Key(KeyEvent::new(code, m)));
                self.buffer.remove(0);
                continue;
            }
            let n = if b < 128 {
                1
            } else if b < 224 {
                2
            } else if b < 240 {
                3
            } else {
                4
            };
            if self.buffer.len() < n {
                break;
            }
            if let Ok(s) = std::str::from_utf8(&self.buffer[..n]) {
                if let Some(c) = s.chars().next() {
                    events.push(Event::Key(KeyEvent::new(
                        KeyCode::Char(c),
                        if c.is_ascii_uppercase() {
                            KeyModifiers::SHIFT
                        } else {
                            KeyModifiers::empty()
                        },
                    )));
                }
            }
            self.buffer.drain(..n);
        }
        events
    }
}
pub fn start(
    tx: Sender<crate::events::AppEvent>,
) -> std::io::Result<Receiver<std::io::Result<Event>>> {
    let mut raw = unsafe {
        libc::open(
            c"/dev/tty".as_ptr(),
            libc::O_RDONLY | libc::O_NONBLOCK | libc::O_CLOEXEC,
        )
    };
    if raw < 0 {
        raw = unsafe {
            libc::open(
                c"/proc/self/fd/0".as_ptr(),
                libc::O_RDONLY | libc::O_NONBLOCK | libc::O_CLOEXEC | libc::O_NOCTTY,
            )
        };
    }
    if raw < 0 {
        return Err(std::io::Error::last_os_error());
    }
    let fd = unsafe { OwnedFd::from_raw_fd(raw) };
    // SAFETY: OwnedFd keeps this descriptor open and stable until AsyncFd drops.
    let fd = unsafe { tokio::io::unix::AsyncFd::register(fd) }?;
    let (out, rx) = channel(64);
    let mut resize = tokio::signal::unix::signal(tokio::signal::unix::SignalKind::window_change())?;
    tokio::spawn(async move {
        let mut decoder = Decoder::default();
        loop {
            tokio::select! {
             ready=fd.readable()=>{let Ok(mut ready)=ready else{break;};let mut bytes=[0;4096];let read=ready.try_io(|inner|{let n=unsafe{libc::read(inner.get_ref().as_raw_fd(),bytes.as_mut_ptr().cast(),bytes.len())};if n<0{Err(std::io::Error::last_os_error())}else{Ok(n as usize)}});if let Ok(Ok(n))=read{if n==0{break;}let previous=decoder.report.clone();for event in decoder.feed(&bytes[..n]){if out.send(Ok(event)).await.is_err(){return;}}if decoder.report!=previous{let _=tx.send(crate::events::AppEvent::TerminalReport(decoder.report.clone())).await;}}},
             _=resize.recv()=>{if let Ok((w,h))=crossterm::terminal::size(){let _=out.send(Ok(Event::Resize(w,h))).await;}},
             _=tokio::time::sleep(std::time::Duration::from_millis(80)),if decoder.pending()=>{if let Some(event)=decoder.escape(){let _=out.send(Ok(event)).await;}else{decoder.buffer.clear();}},
            }
        }
    });
    Ok(rx)
}
