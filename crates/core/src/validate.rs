use crate::model::{AuthMethod, Session};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ValidationIssue {
    pub field: &'static str,
    pub message: String,
}

impl ValidationIssue {
    fn new(field: &'static str, message: impl Into<String>) -> Self {
        Self {
            field,
            message: message.into(),
        }
    }
}

pub fn validate_session(session: &Session) -> Vec<ValidationIssue> {
    let mut issues = Vec::new();

    if session.name.trim().is_empty() {
        issues.push(ValidationIssue::new(
            "name",
            "Informe um nome para a sessão.",
        ));
    }

    if let Some(serial) = &session.serial {
        if let Some(message) = serial.problem() {
            issues.push(ValidationIssue::new("serial", message));
        }
        return issues;
    }

    if session
        .host
        .chars()
        .any(|c| c.is_whitespace() || c == '/' || c == '@')
    {
        issues.push(ValidationIssue::new(
            "host",
            "O host não pode conter espaços, '@' ou '/'.",
        ));
    }

    if session.port == 0 {
        issues.push(ValidationIssue::new(
            "port",
            "A porta deve estar entre 1 e 65535.",
        ));
    }

    if session.user.chars().any(|c| c.is_whitespace() || c == '@') {
        issues.push(ValidationIssue::new(
            "user",
            "O usuário não pode conter espaços ou '@'.",
        ));
    }

    if session.auth.method == AuthMethod::PublicKey {
        match session.auth.key_path.as_ref() {
            None => issues.push(ValidationIssue::new(
                "key_path",
                "Selecione o arquivo da chave privada.",
            )),
            Some(path) if !path.exists() => issues.push(ValidationIssue::new(
                "key_path",
                format!("Chave não encontrada: {}", path.display()),
            )),
            Some(_) => {}
        }
    }

    if let Some(jump) = session.ssh.jump.as_deref() {
        if jump.chars().any(char::is_whitespace) {
            issues.push(ValidationIssue::new(
                "jump",
                "O jump host não pode conter espaços.",
            ));
        }
    }

    issues
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn valid() -> Session {
        let mut session = Session::new("m1", "10.0.0.21");
        session.user = "admin".into();
        session
    }

    #[test]
    fn valid_session_has_no_issues() {
        assert!(validate_session(&valid()).is_empty());
    }

    #[test]
    fn empty_name_is_rejected() {
        let mut session = valid();
        session.name = "  ".into();
        assert_eq!(validate_session(&session)[0].field, "name");
    }

    #[test]
    fn empty_host_is_a_local_terminal() {
        let mut session = valid();
        session.host.clear();
        assert!(validate_session(&session).is_empty());
    }

    #[test]
    fn host_with_at_is_rejected() {
        let mut session = valid();
        session.host = "user@host".into();
        assert!(validate_session(&session).iter().any(|i| i.field == "host"));
    }

    #[test]
    fn public_key_without_path_is_rejected() {
        let mut session = valid();
        session.auth.method = AuthMethod::PublicKey;
        assert!(validate_session(&session)
            .iter()
            .any(|i| i.field == "key_path"));
    }

    #[test]
    fn public_key_with_missing_file_is_rejected() {
        let mut session = valid();
        session.auth.method = AuthMethod::PublicKey;
        session.auth.key_path = Some(PathBuf::from("/definitely/not/here"));
        assert!(validate_session(&session)
            .iter()
            .any(|i| i.field == "key_path"));
    }

    #[test]
    fn serial_session_uses_serial_options_only() {
        let mut session = valid();
        session.host.clear();
        session.serial = Some(crate::model::SerialOptions {
            port: "/dev/ttyUSB0".into(),
            ..Default::default()
        });
        assert!(validate_session(&session).is_empty());
    }

    #[test]
    fn serial_session_rejects_relative_port() {
        let mut session = valid();
        session.serial = Some(crate::model::SerialOptions {
            port: "ttyUSB0".into(),
            ..Default::default()
        });
        let issues = validate_session(&session);
        assert_eq!(issues.len(), 1);
        assert_eq!(issues[0].field, "serial");
    }

    #[test]
    fn serial_session_ignores_ssh_fields() {
        let mut session = valid();
        session.port = 0;
        session.auth.method = AuthMethod::PublicKey;
        session.auth.key_path = None;
        session.ssh.jump = Some("a b".into());
        session.serial = Some(crate::model::SerialOptions {
            port: "/dev/ttyS0".into(),
            ..Default::default()
        });
        assert!(validate_session(&session).is_empty());
    }

    #[test]
    fn jump_host_with_space_is_rejected() {
        let mut session = valid();
        session.ssh.jump = Some("a b".into());
        assert!(validate_session(&session).iter().any(|i| i.field == "jump"));
    }
}
