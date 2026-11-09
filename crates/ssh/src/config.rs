#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConfigEntry {
    pub alias: String,
    pub host_name: Option<String>,
    pub user: Option<String>,
    pub port: Option<u16>,
    pub identity_file: Option<String>,
}

pub fn parse(text: &str) -> Vec<ConfigEntry> {
    let text = join_continuations(text);
    let mut entries = Vec::new();
    let mut current = Vec::new();

    for line in text.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }

        let (key, value) = split_directive(line);
        if key.eq_ignore_ascii_case("Host") {
            entries.append(&mut current);
            current = value
                .split_whitespace()
                .map(unquote)
                .filter(|alias| {
                    !alias.is_empty()
                        && !alias.contains('*')
                        && !alias.contains('?')
                        && !alias.contains('!')
                })
                .map(|alias| ConfigEntry {
                    alias: alias.to_string(),
                    host_name: None,
                    user: None,
                    port: None,
                    identity_file: None,
                })
                .collect();
            continue;
        }

        let value = unquote(value.trim());
        if value.is_empty() {
            continue;
        }

        if key.eq_ignore_ascii_case("HostName") {
            for entry in &mut current {
                if entry.host_name.is_none() {
                    entry.host_name = Some(value.to_string());
                }
            }
        } else if key.eq_ignore_ascii_case("User") {
            for entry in &mut current {
                if entry.user.is_none() {
                    entry.user = Some(value.to_string());
                }
            }
        } else if key.eq_ignore_ascii_case("Port") {
            let port = value.parse::<u16>().ok().filter(|port| *port != 0);
            if let Some(port) = port {
                for entry in &mut current {
                    if entry.port.is_none() {
                        entry.port = Some(port);
                    }
                }
            }
        } else if key.eq_ignore_ascii_case("IdentityFile") {
            for entry in &mut current {
                if entry.identity_file.is_none() {
                    entry.identity_file = Some(value.to_string());
                }
            }
        }
    }

    entries.append(&mut current);
    entries
}

pub fn read_default() -> Vec<ConfigEntry> {
    let Some(home) = std::env::var_os("HOME") else {
        return Vec::new();
    };
    let path = std::path::Path::new(&home).join(".ssh").join("config");

    std::fs::read_to_string(path)
        .map(|text| parse(&text))
        .unwrap_or_default()
}

fn join_continuations(text: &str) -> String {
    let mut joined = String::new();
    let mut continuing = false;

    for line in text.lines() {
        let line = if continuing { line.trim_start() } else { line };
        if let Some(prefix) = line.strip_suffix('\\') {
            joined.push_str(prefix);
            continuing = true;
        } else {
            joined.push_str(line);
            joined.push('\n');
            continuing = false;
        }
    }

    joined
}

fn split_directive(line: &str) -> (&str, &str) {
    let separator = line
        .find(|c: char| c.is_whitespace() || c == '=')
        .unwrap_or(line.len());
    let key = &line[..separator];
    let value = line[separator..].trim_start();
    let value = value.strip_prefix('=').unwrap_or(value).trim_start();

    (key, value)
}

fn unquote(value: &str) -> &str {
    value
        .strip_prefix('"')
        .and_then(|value| value.strip_suffix('"'))
        .unwrap_or(value)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_two_hosts_with_connection_settings() {
        let entries = parse(
            "Host dev\n\
             \tHostName dev.example.com\n\
             \tUser alice\n\
             \tPort 2222\n\
             \tIdentityFile ~/.ssh/id_ed25519\n\
             \n\
             Host prod\n\
             \tHostName prod.example.com\n\
             \tUser bob\n\
             \tPort 22\n\
             \tIdentityFile /home/bob/.ssh/id_rsa\n",
        );

        assert_eq!(
            entries,
            vec![
                ConfigEntry {
                    alias: "dev".into(),
                    host_name: Some("dev.example.com".into()),
                    user: Some("alice".into()),
                    port: Some(2222),
                    identity_file: Some("~/.ssh/id_ed25519".into()),
                },
                ConfigEntry {
                    alias: "prod".into(),
                    host_name: Some("prod.example.com".into()),
                    user: Some("bob".into()),
                    port: Some(22),
                    identity_file: Some("/home/bob/.ssh/id_rsa".into()),
                },
            ]
        );
    }

    #[test]
    fn keywords_are_case_insensitive_and_accept_equals_separator() {
        let entries = parse(
            "hOsT=jump\n\
             hOsTnAmE = jump.example.com\n\
             uSeR=admin\n\
             pOrT = 2200\n\
             iDeNtItYfIlE=~/.ssh/jump\n",
        );

        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].host_name.as_deref(), Some("jump.example.com"));
        assert_eq!(entries[0].user.as_deref(), Some("admin"));
        assert_eq!(entries[0].port, Some(2200));
        assert_eq!(entries[0].identity_file.as_deref(), Some("~/.ssh/jump"));
    }

    #[test]
    fn host_with_multiple_aliases_preserves_their_order() {
        let entries = parse("Host one two three\n HostName shared.example.com\n");

        assert_eq!(
            entries
                .iter()
                .map(|entry| entry.alias.as_str())
                .collect::<Vec<_>>(),
            vec!["one", "two", "three"]
        );
        assert!(entries
            .iter()
            .all(|entry| entry.host_name.as_deref() == Some("shared.example.com")));
    }

    #[test]
    fn wildcard_and_negated_aliases_are_ignored() {
        let entries = parse(
            "Host * *.example.com good? !negated web\n\
             Host db\n\
             HostName db.example.com\n",
        );

        assert_eq!(
            entries
                .iter()
                .map(|entry| entry.alias.as_str())
                .collect::<Vec<_>>(),
            vec!["web", "db"]
        );
    }

    #[test]
    fn invalid_ports_are_ignored() {
        let entries = parse(
            "Host invalid\n\
             Port 0\n\
             Port 65536\n\
             Port invalid\n\
             Host valid\n\
             Port 65535\n",
        );

        assert_eq!(entries[0].port, None);
        assert_eq!(entries[1].port, Some(65535));
    }

    #[test]
    fn surrounding_double_quotes_are_removed() {
        let entries = parse(
            "Host \"dev\"\n\
             HostName \"example.com\"\n\
             User \"alice\"\n\
             Port \"2222\"\n\
             IdentityFile \"~/.ssh/id key\"\n",
        );

        assert_eq!(entries[0].alias, "dev");
        assert_eq!(entries[0].host_name.as_deref(), Some("example.com"));
        assert_eq!(entries[0].user.as_deref(), Some("alice"));
        assert_eq!(entries[0].port, Some(2222));
        assert_eq!(entries[0].identity_file.as_deref(), Some("~/.ssh/id key"));
    }

    #[test]
    fn continuation_lines_are_joined_before_parsing() {
        let entries = parse("Host dev \\\n  development\n  HostName exam\\\n  ple.com\n");

        assert_eq!(
            entries
                .iter()
                .map(|entry| entry.alias.as_str())
                .collect::<Vec<_>>(),
            vec!["dev", "development"]
        );
        assert!(entries
            .iter()
            .all(|entry| entry.host_name.as_deref() == Some("example.com")));
    }

    #[test]
    fn directives_before_the_first_host_are_ignored() {
        let entries = parse(
            "HostName ignored.example.com\n\
             User ignored\n\
             Port 2222\n\
             IdentityFile ~/.ssh/ignored\n\
             Host real\n\
             User kept\n",
        );

        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].alias, "real");
        assert_eq!(entries[0].host_name, None);
        assert_eq!(entries[0].user.as_deref(), Some("kept"));
        assert_eq!(entries[0].port, None);
        assert_eq!(entries[0].identity_file, None);
    }

    #[test]
    fn first_duplicate_value_wins_including_identity_file() {
        let entries = parse(
            "Host dev\n\
             HostName first.example.com\n\
             HostName second.example.com\n\
             User first\n\
             User second\n\
             Port 2222\n\
             Port 22\n\
             IdentityFile ~/.ssh/first\n\
             IdentityFile ~/.ssh/second\n",
        );

        assert_eq!(entries[0].host_name.as_deref(), Some("first.example.com"));
        assert_eq!(entries[0].user.as_deref(), Some("first"));
        assert_eq!(entries[0].port, Some(2222));
        assert_eq!(entries[0].identity_file.as_deref(), Some("~/.ssh/first"));
    }
}
