use lt_core::Session;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct QuickConnect {
    pub user: Option<String>,
    pub host: String,
    pub port: u16,
}

impl QuickConnect {
    pub fn to_session(&self) -> Session {
        let mut session = Session::new(&self.host, &self.host);
        session.user = self.user.clone().unwrap_or_default();
        session.port = self.port;
        session
    }
}

pub fn parse(input: &str) -> Option<QuickConnect> {
    let input = input.trim();
    if input.is_empty() || input.chars().any(char::is_whitespace) {
        return None;
    }

    let (user, rest) = match input.rsplit_once('@') {
        Some((user, rest)) if !user.is_empty() && !rest.is_empty() => {
            (Some(user.to_string()), rest)
        }
        Some(_) => return None,
        None => (None, input),
    };

    let (host, port) = if let Some(stripped) = rest.strip_prefix('[') {
        let (host, rest) = stripped.split_once(']')?;
        let port = match rest {
            "" => 22,
            _ => parse_port(rest.strip_prefix(':')?)?,
        };
        (host.to_string(), port)
    } else if rest.matches(':').count() == 1 {
        let (host, port) = rest.rsplit_once(':')?;
        (host.to_string(), parse_port(port)?)
    } else {
        (rest.to_string(), 22)
    };

    if host.is_empty() || host.starts_with('-') {
        return None;
    }

    Some(QuickConnect { user, host, port })
}

fn parse_port(value: &str) -> Option<u16> {
    value.parse::<u16>().ok().filter(|port| *port > 0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn plain_host() {
        let parsed = parse("10.0.0.21").unwrap();
        assert_eq!(parsed.host, "10.0.0.21");
        assert_eq!(parsed.port, 22);
        assert_eq!(parsed.user, None);
    }

    #[test]
    fn user_and_host() {
        let parsed = parse("admin@10.0.0.21").unwrap();
        assert_eq!(parsed.user.as_deref(), Some("admin"));
        assert_eq!(parsed.host, "10.0.0.21");
    }

    #[test]
    fn host_and_port() {
        let parsed = parse("10.0.0.21:2222").unwrap();
        assert_eq!(parsed.port, 2222);
        assert_eq!(parsed.host, "10.0.0.21");
    }

    #[test]
    fn user_host_and_port() {
        let parsed = parse("admin@10.0.0.21:2222").unwrap();
        assert_eq!(parsed.user.as_deref(), Some("admin"));
        assert_eq!(parsed.host, "10.0.0.21");
        assert_eq!(parsed.port, 2222);
    }

    #[test]
    fn ipv6_with_brackets_and_port() {
        let parsed = parse("root@[::1]:2200").unwrap();
        assert_eq!(parsed.host, "::1");
        assert_eq!(parsed.port, 2200);
        assert_eq!(parsed.user.as_deref(), Some("root"));
    }

    #[test]
    fn bare_ipv6_keeps_host() {
        let parsed = parse("::1").unwrap();
        assert_eq!(parsed.host, "::1");
        assert_eq!(parsed.port, 22);
    }

    #[test]
    fn rejects_invalid_input() {
        assert!(parse("").is_none());
        assert!(parse("  ").is_none());
        assert!(parse("host with space").is_none());
        assert!(parse("host:0").is_none());
        assert!(parse("host:99999").is_none());
        assert!(parse("@host").is_none());
        assert!(parse("-oProxyCommand=evil").is_none());
    }

    #[test]
    fn session_uses_host_as_name() {
        let session = parse("admin@host:2200").unwrap().to_session();
        assert_eq!(session.name, "host");
        assert_eq!(session.user, "admin");
        assert_eq!(session.port, 2200);
    }
}
