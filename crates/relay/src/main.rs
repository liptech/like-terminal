use std::ffi::CString;
use std::io::Write;
use std::ptr;

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
            "--" => {
                command.extend(args[index + 1..].iter().cloned());
                break;
            }
            _ => index += 1,
        }
    }

    if command.is_empty() {
        eprintln!(
            "uso: like-terminal-relay [--rules 'Chave=#rrggbb[:bold][:nocase];...'] [--rules-file arquivo] [--ips] -- comando [args]"
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

    relay(master, pid, Filter::new(rules, color_ips), size);
}

fn relay(master: libc::c_int, pid: libc::pid_t, mut filter: Filter, mut size: libc::winsize) {
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
}
