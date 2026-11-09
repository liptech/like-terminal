use std::ffi::CString;
use std::fs::File;
use std::io::Write;
use std::ptr;
use std::sync::atomic::{AtomicBool, Ordering};

const ESC: u8 = 0x1b;

#[derive(Clone)]
struct Rule {
    keyword: Vec<u8>,
    color: String,
    bold: bool,
    nocase: bool,
}

struct Filter {
    rules: Vec<Rule>,
    carry: Vec<u8>,
    max_len: usize,
    color_ips: bool,
    sgr: Vec<u8>,
}

impl Filter {
    fn new(rules: Vec<Rule>, color_ips: bool) -> Self {
        let max_len = rules
            .iter()
            .map(|rule| rule.keyword.len())
            .max()
            .unwrap_or(1);
        Self {
            rules,
            carry: Vec::new(),
            max_len,
            color_ips,
            sgr: Vec::new(),
        }
    }

    fn feed(&mut self, data: &[u8]) -> Vec<u8> {
        let mut buffer = std::mem::take(&mut self.carry);
        buffer.extend_from_slice(data);
        let mut out = Vec::with_capacity(buffer.len() + 32);
        let mut safe = buffer.len().saturating_sub(self.max_len.saturating_sub(1));
        while safe > 0 && safe < buffer.len() && (buffer[safe] & 0xc0) == 0x80 {
            safe -= 1;
        }
        let mut index = 0;

        while index < safe {
            if buffer[index] == ESC {
                let end = escape_end(&buffer, index);
                out.extend_from_slice(&buffer[index..end]);
                self.track_sgr(&buffer[index..end]);
                index = end;
                continue;
            }
            if let Some((len, code)) = self.rule_at(&buffer, index, safe) {
                out.extend_from_slice(code.as_bytes());
                out.extend_from_slice(&buffer[index..index + len]);
                out.extend_from_slice(b"\x1b[0m");
                out.extend_from_slice(&self.sgr);
                index += len;
                continue;
            }
            out.push(buffer[index]);
            index += 1;
        }

        self.carry = buffer[index..].to_vec();
        out
    }

    fn track_sgr(&mut self, sequence: &[u8]) {
        if sequence.starts_with(b"\x1b[") && sequence.ends_with(b"m") {
            if sequence.len() <= 3 || &sequence[2..sequence.len() - 1] == b"0" {
                self.sgr.clear();
            } else {
                self.sgr = sequence.to_vec();
            }
        }
    }

    fn flush(&mut self) -> Vec<u8> {
        std::mem::take(&mut self.carry)
    }

    fn rule_at(&self, buffer: &[u8], index: usize, safe: usize) -> Option<(usize, String)> {
        if self.color_ips {
            if let Some(len) = ip_at(buffer, index) {
                return Some((len, "\x1b[38;2;250;189;47m".to_string()));
            }
        }
        for rule in &self.rules {
            let end = index + rule.keyword.len();
            if end > buffer.len() || index > safe {
                continue;
            }
            let matches = if rule.nocase {
                buffer[index..end].eq_ignore_ascii_case(&rule.keyword)
            } else {
                &buffer[index..end] == rule.keyword.as_slice()
            };
            if matches {
                let mut code = String::from("\x1b[");
                if rule.bold {
                    code.push_str("1;");
                }
                code.push_str("38;2;");
                code.push_str(&rule.color);
                code.push('m');
                return Some((rule.keyword.len(), code));
            }
        }
        None
    }
}

static LOG_TOGGLE: AtomicBool = AtomicBool::new(false);

extern "C" fn toggle_log(_signal: libc::c_int) {
    LOG_TOGGLE.store(true, Ordering::Relaxed);
}

struct Logger {
    file: Option<File>,
    path: Option<String>,
    timestamps: bool,
    enabled: bool,
    at_line_start: bool,
    pending: Vec<u8>,
}

impl Logger {
    fn closed() -> Self {
        Self {
            file: None,
            path: None,
            timestamps: false,
            enabled: false,
            at_line_start: true,
            pending: Vec::new(),
        }
    }

    fn open(path: &str, timestamps: bool, enabled: bool) -> std::io::Result<Self> {
        unsafe {
            let handler: extern "C" fn(libc::c_int) = toggle_log;
            libc::signal(libc::SIGUSR1, handler as *const () as libc::sighandler_t);
        }
        Ok(Self {
            file: None,
            path: Some(path.to_string()),
            timestamps,
            enabled,
            at_line_start: true,
            pending: Vec::new(),
        })
    }

    fn write(&mut self, data: &[u8]) {
        if self.path.is_none() {
            return;
        }
        let mut buffer = std::mem::take(&mut self.pending);
        buffer.extend_from_slice(data);
        let mut index = 0;
        while index < buffer.len() {
            let byte = buffer[index];
            if byte == ESC {
                let Some(end) = log_sequence_end(&buffer, index) else {
                    break;
                };
                index = end;
                continue;
            }
            if byte == b'\r' {
                index += 1;
                continue;
            }
            if byte == b'\n' {
                self.write_text(b"\n");
                index += 1;
                continue;
            }
            let start = index;
            while index < buffer.len()
                && buffer[index] != b'\n'
                && buffer[index] != b'\r'
                && buffer[index] != ESC
            {
                index += 1;
            }
            self.write_text(&buffer[start..index]);
        }
        self.pending = buffer[index..].to_vec();
    }

    fn write_text(&mut self, text: &[u8]) {
        if LOG_TOGGLE.swap(false, Ordering::Relaxed) {
            self.enabled = !self.enabled;
        }
        if !self.enabled {
            return;
        }
        if self.file.is_none() {
            let Some(path) = self.path.as_deref() else {
                return;
            };
            if let Some(parent) = std::path::Path::new(path).parent() {
                let _ = std::fs::create_dir_all(parent);
            }
            match File::create(path) {
                Ok(file) => self.file = Some(file),
                Err(_) => return,
            }
        }
        let Some(file) = self.file.as_mut() else {
            return;
        };
        if self.at_line_start && self.timestamps && text != b"\n" {
            let _ = file.write_all(iso_now().as_bytes());
        }
        let _ = file.write_all(text);
        self.at_line_start = text.ends_with(b"\n");
    }

    fn flush(&mut self) {
        let pending = std::mem::take(&mut self.pending);
        if !pending.is_empty() {
            self.write_text(&pending);
        }
        if let Some(file) = self.file.as_mut() {
            let _ = file.flush();
        }
    }
}

fn iso_now() -> String {
    unsafe {
        let mut now: libc::time_t = 0;
        libc::time(&mut now);
        let mut parts: libc::tm = std::mem::zeroed();
        libc::localtime_r(&now, &mut parts);
        format!(
            "{:04}-{:02}-{:02}T{:02}:{:02}:{:02} ",
            parts.tm_year + 1900,
            parts.tm_mon + 1,
            parts.tm_mday,
            parts.tm_hour,
            parts.tm_min,
            parts.tm_sec
        )
    }
}

fn log_sequence_end(buffer: &[u8], start: usize) -> Option<usize> {
    match buffer.get(start + 1) {
        Some(b'[') => {
            let mut index = start + 2;
            while index < buffer.len() {
                let byte = buffer[index];
                index += 1;
                if (0x40..=0x7e).contains(&byte) {
                    return Some(index);
                }
            }
            None
        }
        Some(b']') => {
            let mut index = start + 2;
            while index < buffer.len() {
                if buffer[index] == 0x07 {
                    return Some(index + 1);
                }
                if buffer[index] == ESC && buffer.get(index + 1) == Some(&b'\\') {
                    return Some(index + 2);
                }
                index += 1;
            }
            None
        }
        Some(_) => Some((start + 2).min(buffer.len())),
        None => None,
    }
}

fn escape_end(buffer: &[u8], start: usize) -> usize {
    let mut index = start + 1;
    if buffer.get(index) == Some(&b'[') {
        index += 1;
        while index < buffer.len() {
            let byte = buffer[index];
            index += 1;
            if (0x40..=0x7e).contains(&byte) {
                break;
            }
        }
    }
    index.min(buffer.len())
}

fn ip_at(buffer: &[u8], index: usize) -> Option<usize> {
    if index > 0 {
        let previous = buffer[index - 1];
        if previous.is_ascii_alphanumeric() || previous == b'.' {
            return None;
        }
    }
    let mut position = index;
    for group in 0..4 {
        let start = position;
        while position < buffer.len() && buffer[position].is_ascii_digit() && position - start < 3 {
            position += 1;
        }
        if position == start {
            return None;
        }
        if group < 3 {
            if buffer.get(position) != Some(&b'.') {
                return None;
            }
            position += 1;
        }
    }
    if position < buffer.len() {
        let next = buffer[position];
        if next.is_ascii_alphanumeric() || next == b'.' {
            return None;
        }
    }
    Some(position - index)
}

fn parse_rules(spec: &str) -> Vec<Rule> {
    let mut rules = Vec::new();
    for entry in spec.split([';', '\n']) {
        let Some((keyword, value)) = entry.split_once('=') else {
            continue;
        };
        if keyword.is_empty() {
            continue;
        }
        let mut parts = value.split(':');
        let color = parts.next().unwrap_or_default();
        let flags: Vec<&str> = parts.collect();
        let Some(rgb) = parse_hex(color) else {
            continue;
        };
        rules.push(Rule {
            keyword: keyword.trim().as_bytes().to_vec(),
            color: rgb,
            bold: flags.contains(&"bold"),
            nocase: flags.contains(&"nocase"),
        });
    }
    rules
}

fn parse_hex(value: &str) -> Option<String> {
    let hex = value.strip_prefix('#')?;
    if hex.len() != 6 || !hex.chars().all(|c| c.is_ascii_hexdigit()) {
        return None;
    }
    let r = u8::from_str_radix(&hex[0..2], 16).ok()?;
    let g = u8::from_str_radix(&hex[2..4], 16).ok()?;
    let b = u8::from_str_radix(&hex[4..6], 16).ok()?;
    Some(format!("{r};{g};{b}"))
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let mut rules = Vec::new();
    let mut color_ips = false;
    let mut log_path: Option<String> = None;
    let mut log_timestamps = true;
    let mut log_enabled = true;
    let mut command = Vec::new();
    let mut index = 1;
    while index < args.len() {
        match args[index].as_str() {
            "--rules" => {
                if let Some(spec) = args.get(index + 1) {
                    rules.extend(parse_rules(spec));
                }
                index += 2;
            }
            "--rules-file" => {
                if let Some(path) = args.get(index + 1) {
                    if let Ok(contents) = std::fs::read_to_string(path) {
                        rules.extend(parse_rules(&contents));
                    }
                }
                index += 2;
            }
            "--ips" => {
                color_ips = true;
                index += 1;
            }
            "--log" => {
                log_path = args.get(index + 1).cloned();
                index += 2;
            }
            "--log-raw" => {
                log_timestamps = false;
                index += 1;
            }
            "--log-paused" => {
                log_enabled = false;
                index += 1;
            }
            "--" => {
                command.extend(args[index + 1..].iter().cloned());
                break;
            }
            _ => index += 1,
        }
    }

    if command.is_empty() {
        eprintln!(
            "uso: like-terminal-relay [--rules 'Chave=#rrggbb[:bold][:nocase];...'] [--rules-file arquivo] [--ips] [--log arquivo [--log-raw] [--log-paused]] -- comando [args]"
        );
        std::process::exit(2);
    }

    let mut size = libc::winsize {
        ws_row: 24,
        ws_col: 80,
        ws_xpixel: 0,
        ws_ypixel: 0,
    };
    unsafe {
        libc::ioctl(libc::STDIN_FILENO, libc::TIOCGWINSZ, &mut size);
    }

    let mut master: libc::c_int = 0;
    let pid = unsafe { libc::forkpty(&mut master, ptr::null_mut(), ptr::null(), &size) };
    if pid < 0 {
        eprintln!("like-terminal-relay: falha ao abrir pty");
        std::process::exit(1);
    }

    if pid == 0 {
        unsafe {
            let mut termios: libc::termios = std::mem::zeroed();
            if libc::tcgetattr(libc::STDIN_FILENO, &mut termios) == 0 {
                libc::cfmakeraw(&mut termios);
                libc::tcsetattr(libc::STDIN_FILENO, libc::TCSANOW, &termios);
            }
        }
        let program = CString::new(command[0].clone()).unwrap_or_default();
        let mut argv: Vec<CString> = command
            .iter()
            .map(|arg| CString::new(arg.as_str()).unwrap_or_default())
            .collect();
        let mut pointers: Vec<*const libc::c_char> =
            argv.iter_mut().map(|arg| arg.as_ptr()).collect();
        pointers.push(ptr::null());
        unsafe {
            libc::execvp(program.as_ptr(), pointers.as_ptr());
        }
        std::process::exit(127);
    }

    let mut logger = match log_path {
        Some(path) => match Logger::open(&path, log_timestamps, log_enabled) {
            Ok(logger) => logger,
            Err(error) => {
                eprintln!("like-terminal-relay: falha ao abrir log {path}: {error}");
                Logger::closed()
            }
        },
        None => Logger::closed(),
    };

    relay(
        master,
        pid,
        Filter::new(rules, color_ips),
        size,
        &mut logger,
    );
}

fn relay(
    master: libc::c_int,
    pid: libc::pid_t,
    mut filter: Filter,
    mut size: libc::winsize,
    logger: &mut Logger,
) {
    let stdout = std::io::stdout();
    let mut output = stdout.lock();
    let mut status: libc::c_int = 0;
    let mut master_open = true;
    let mut stdin_open = true;

    loop {
        let mut fds = Vec::with_capacity(2);
        if stdin_open {
            fds.push(libc::pollfd {
                fd: libc::STDIN_FILENO,
                events: libc::POLLIN,
                revents: 0,
            });
        }
        if master_open {
            fds.push(libc::pollfd {
                fd: master,
                events: libc::POLLIN,
                revents: 0,
            });
        }
        if fds.is_empty() {
            break;
        }

        let ready = unsafe { libc::poll(fds.as_mut_ptr(), fds.len() as libc::nfds_t, 100) };

        if ready > 0 {
            for entry in &fds {
                if entry.revents == 0 {
                    continue;
                }
                if entry.fd == libc::STDIN_FILENO {
                    let mut buffer = [0u8; 4096];
                    let read = unsafe {
                        libc::read(libc::STDIN_FILENO, buffer.as_mut_ptr().cast(), buffer.len())
                    };
                    if read > 0 {
                        unsafe {
                            libc::write(master, buffer.as_ptr().cast(), read as usize);
                        }
                        if size_changed(master, &mut size) {
                            unsafe {
                                libc::kill(pid, libc::SIGWINCH);
                            }
                        }
                    } else {
                        stdin_open = false;
                    }
                } else {
                    let mut buffer = [0u8; 8192];
                    let read =
                        unsafe { libc::read(master, buffer.as_mut_ptr().cast(), buffer.len()) };
                    if read > 0 {
                        logger.write(&buffer[..read as usize]);
                        let bytes = filter.feed(&buffer[..read as usize]);
                        if !bytes.is_empty() {
                            let _ = output.write_all(&bytes);
                            let _ = output.flush();
                        }
                    } else {
                        master_open = false;
                    }
                }
            }
        } else {
            let bytes = filter.flush();
            if !bytes.is_empty() {
                let _ = output.write_all(&bytes);
                let _ = output.flush();
            }
            if size_changed(master, &mut size) {
                unsafe {
                    libc::kill(pid, libc::SIGWINCH);
                }
            }
        }

        if !master_open {
            break;
        }
    }

    let _ = unsafe { libc::waitpid(pid, &mut status, 0) };
    logger.flush();
    let bytes = filter.flush();
    if !bytes.is_empty() {
        let _ = output.write_all(&bytes);
    }
    let _ = output.flush();
    let code = if libc::WIFEXITED(status) {
        libc::WEXITSTATUS(status)
    } else {
        128 + libc::WTERMSIG(status)
    };
    std::process::exit(code);
}

fn size_changed(master: libc::c_int, size: &mut libc::winsize) -> bool {
    let mut current = libc::winsize {
        ws_row: 0,
        ws_col: 0,
        ws_xpixel: 0,
        ws_ypixel: 0,
    };
    unsafe {
        libc::ioctl(libc::STDIN_FILENO, libc::TIOCGWINSZ, &mut current);
        if current.ws_row == size.ws_row && current.ws_col == size.ws_col {
            return false;
        }
        libc::ioctl(master, libc::TIOCSWINSZ, &current);
    }
    *size = current;
    true
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rule(keyword: &str, color: &str) -> Rule {
        Rule {
            keyword: keyword.as_bytes().to_vec(),
            color: color.to_string(),
            bold: false,
            nocase: true,
        }
    }

    #[test]
    fn colors_keywords_across_chunks() {
        let mut filter = Filter::new(vec![rule("Error", "255;0;0")], false);
        let mut out = filter.feed(b"ok Err");
        out.extend(filter.feed(b"or end\n"));
        let text = String::from_utf8_lossy(&out);
        assert!(text.contains("\x1b[38;2;255;0;0mError\x1b[0m"), "{text:?}");
    }

    #[test]
    fn ignores_keywords_inside_escape_sequences() {
        let mut filter = Filter::new(vec![rule("31", "0;255;0")], false);
        let mut out = filter.feed(b"\x1b[31mok\x1b[0m\n");
        out.extend(filter.flush());
        let text = String::from_utf8_lossy(&out);
        assert_eq!(text, "\x1b[31mok\x1b[0m\n");
    }

    #[test]
    fn restores_previous_color_after_match() {
        let mut filter = Filter::new(vec![rule("Error", "255;0;0")], false);
        let out = filter.feed(b"\x1b[32mError\x1b[0m");
        let text = String::from_utf8_lossy(&out);
        assert!(text.contains("Error\x1b[0m\x1b[32m"), "{text:?}");
    }

    #[test]
    fn colors_ip_addresses() {
        let mut filter = Filter::new(Vec::new(), true);
        let out = filter.feed(b"host 10.255.255.242 up\n");
        let text = String::from_utf8_lossy(&out);
        assert!(text.contains("38;2;250;189;47m10.255.255.242"));
    }

    #[test]
    fn parses_rule_specs() {
        let rules = parse_rules("Error=#ff0000:bold;Info=#00ff00");
        assert_eq!(rules.len(), 2);
        assert!(rules[0].bold);
        assert!(!rules[0].nocase);
        assert!(!rules[1].nocase);
        assert_eq!(rules[0].color, "255;0;0");
    }

    #[test]
    fn respects_case_when_sensitive() {
        let mut sensitive = Rule {
            keyword: b"Error".to_vec(),
            color: "255;0;0".to_string(),
            bold: false,
            nocase: false,
        };
        let mut filter = Filter::new(vec![sensitive.clone()], false);
        let out = filter.feed(b"error ERROR Error\n");
        let text = String::from_utf8_lossy(&out);
        assert_eq!(text.matches("38;2;255;0;0").count(), 1);

        sensitive.nocase = true;
        let mut filter = Filter::new(vec![sensitive], false);
        let out = filter.feed(b"error ERROR Error\n");
        let text = String::from_utf8_lossy(&out);
        assert_eq!(text.matches("38;2;255;0;0").count(), 3);
    }

    #[test]
    fn keeps_multibyte_characters_intact() {
        let mut filter = Filter::new(vec![rule("xyz", "0;255;0")], false);
        let data = "Ação concluída ✓".as_bytes();
        let mut out = Vec::new();
        for chunk in data.chunks(3) {
            out.extend(filter.feed(chunk));
        }
        out.extend(filter.flush());
        assert_eq!(String::from_utf8_lossy(&out), "Ação concluída ✓");
    }

    #[test]
    fn keeps_multibyte_characters_at_every_split_point() {
        let text = "Ação concluída ✓ — fim";
        let data = text.as_bytes();
        for split in 1..data.len() {
            let mut filter = Filter::new(vec![rule("fim", "0;255;0")], false);
            let mut out = filter.feed(&data[..split]);
            out.extend(filter.feed(&data[split..]));
            out.extend(filter.flush());
            let rendered = String::from_utf8_lossy(&out).to_string();
            let plain = rendered
                .replace("\x1b[38;2;0;255;0m", "")
                .replace("\x1b[0m", "");
            assert_eq!(plain, text, "split em {split}");
        }
    }

    #[test]
    fn colors_accented_keyword_split_between_chunks() {
        let mut filter = Filter::new(vec![rule("concluída", "0;255;0")], false);
        let data = "Ação concluída ✓".as_bytes();
        let mut out = filter.feed(&data[..8]);
        out.extend(filter.feed(&data[8..]));
        out.extend(filter.flush());
        let text = String::from_utf8_lossy(&out);
        assert!(
            text.contains("\x1b[38;2;0;255;0mconcluída\x1b[0m"),
            "{text:?}"
        );
    }

    #[test]
    fn handles_chunk_ending_on_continuation_byte() {
        let mut filter = Filter::new(vec![rule("ção", "0;255;0")], false);
        let data = "Ação".as_bytes();
        let mut out = filter.feed(&data[..3]);
        out.extend(filter.feed(&data[3..]));
        out.extend(filter.flush());
        let text = String::from_utf8_lossy(&out);
        assert_eq!(
            text.replace("\x1b[38;2;0;255;0m", "")
                .replace("\x1b[0m", ""),
            "Ação"
        );
        assert!(text.contains("ção"), "{text:?}");
    }

    #[test]
    fn passes_output_through_without_rules() {
        let mut filter = Filter::new(Vec::new(), false);
        let mut out = filter.feed(b"\x1b[32mok\x1b[0m\n");
        out.extend(filter.flush());
        assert_eq!(String::from_utf8_lossy(&out), "\x1b[32mok\x1b[0m\n");
    }

    fn temp_path(name: &str) -> String {
        let unique = format!(
            "lt-relay-test-{}-{}-{name}.log",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_nanos())
                .unwrap_or(0)
        );
        std::env::temp_dir()
            .join("lt-relay-tests")
            .join(unique)
            .display()
            .to_string()
    }

    #[test]
    fn log_strips_escape_sequences() {
        let path = temp_path("plain");
        let mut logger = Logger::open(&path, false, true).unwrap();
        logger.write(b"\x1b[32mok\x1b[0m\r\nsegunda linha\n");
        logger.flush();
        let content = std::fs::read_to_string(&path).unwrap();
        assert_eq!(content, "ok\nsegunda linha\n");
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn log_prefixes_iso_timestamps() {
        let path = temp_path("stamps");
        let mut logger = Logger::open(&path, true, true).unwrap();
        logger.write(b"a\nb\n");
        logger.flush();
        let content = std::fs::read_to_string(&path).unwrap();
        let lines: Vec<&str> = content.lines().collect();
        assert_eq!(lines.len(), 2);
        for line in lines {
            assert_eq!(line.len(), 20 + 1, "{line:?}");
            assert_eq!(&line[4..5], "-");
            assert_eq!(&line[10..11], "T");
        }
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn log_waits_for_enable_signal() {
        let path = temp_path("paused");
        let mut logger = Logger::open(&path, false, false).unwrap();
        logger.write(b"inativo\n");
        logger.flush();
        assert!(!std::path::Path::new(&path).exists());
        logger.enabled = true;
        logger.write(b"ativo\n");
        logger.flush();
        assert_eq!(std::fs::read_to_string(&path).unwrap(), "ativo\n");
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn log_strips_osc_titles() {
        let path = temp_path("osc");
        let mut logger = Logger::open(&path, false, true).unwrap();
        logger.write(b"\x1b]0;titulo\x07ok\n");
        logger.write(b"\x1b]2;outro\x1b\\fim\n");
        logger.flush();
        assert_eq!(std::fs::read_to_string(&path).unwrap(), "ok\nfim\n");
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn log_keeps_split_escape_across_chunks() {
        let path = temp_path("escape");
        let mut logger = Logger::open(&path, false, true).unwrap();
        logger.write(b"\x1b[3");
        logger.write(b"2mok\n");
        logger.flush();
        assert_eq!(std::fs::read_to_string(&path).unwrap(), "ok\n");
        let _ = std::fs::remove_file(&path);
    }
}
