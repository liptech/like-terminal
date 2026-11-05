use std::path::PathBuf;

use lt_core::{AuthMethod, Session};
use uuid::Uuid;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AskpassKind {
    Password,
    Passphrase,
}

impl AskpassKind {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Password => "password",
            Self::Passphrase => "passphrase",
        }
    }
}

#[derive(Debug, Clone)]
pub struct AskpassSpec {
    pub program: PathBuf,
    pub session_id: Uuid,
    pub kind: AskpassKind,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CommandSpec {
    pub program: String,
    pub args: Vec<String>,
    pub env: Vec<(String, String)>,
}

impl CommandSpec {
    pub fn argv(&self) -> Vec<String> {
        let mut argv = Vec::with_capacity(self.args.len() + 1);
        argv.push(self.program.clone());
        argv.extend(self.args.iter().cloned());
        argv
    }
}

#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum BuildError {
    #[error("host inválido")]
    InvalidHost,
    #[error("usuário inválido")]
    InvalidUser,
    #[error("chave privada não informada")]
    MissingKey,
}

pub fn build(session: &Session, askpass: Option<&AskpassSpec>) -> Result<CommandSpec, BuildError> {
    if session.is_local() {
        let program = std::env::var("SHELL").unwrap_or_else(|_| "/bin/bash".to_string());
        return Ok(CommandSpec {
            program,
            args: Vec::new(),
            env: Vec::new(),
        });
    }
    if session.host.trim().is_empty()
        || session.host.starts_with('-')
        || session
            .host
            .chars()
            .any(|c| c.is_whitespace() || c == '/' || c == '@')
    {
        return Err(BuildError::InvalidHost);
    }
    if session
        .user
        .chars()
        .any(|c| c.is_whitespace() || c == '@' || c == '-')
    {
        return Err(BuildError::InvalidUser);
    }

    let mut args: Vec<String> = vec!["-tt".into()];

    args.push("-p".into());
    args.push(session.port.to_string());

    if !session.user.is_empty() {
        args.push("-l".into());
        args.push(session.user.clone());
    }

    args.push("-o".into());
    args.push("ConnectTimeout=15".into());

    if session.ssh.keepalive > 0 {
        args.push("-o".into());
        args.push(format!("ServerAliveInterval={}", session.ssh.keepalive));
        args.push("-o".into());
        args.push("ServerAliveCountMax=3".into());
    }

    match session.auth.method {
        AuthMethod::Password => {
            args.push("-o".into());
            args.push("PreferredAuthentications=password,keyboard-interactive,publickey".into());
        }
        AuthMethod::KeyboardInteractive => {
            args.push("-o".into());
            args.push("PreferredAuthentications=keyboard-interactive,password".into());
        }
        AuthMethod::PublicKey => {
            let key = session
                .auth
                .key_path
                .as_ref()
                .ok_or(BuildError::MissingKey)?;
            args.push("-i".into());
            args.push(key.display().to_string());
            args.push("-o".into());
            args.push("IdentitiesOnly=yes".into());
            args.push("-o".into());
            args.push("PreferredAuthentications=publickey".into());
        }
        AuthMethod::Agent => {
            args.push("-o".into());
            args.push("PreferredAuthentications=publickey".into());
        }
    }

    if let Some(jump) = session
        .ssh
        .jump
        .as_deref()
        .map(str::trim)
        .filter(|jump| !jump.is_empty())
    {
        args.push("-J".into());
        args.push(jump.to_string());
    }

    if session.ssh.compression {
        args.push("-o".into());
        args.push("Compression=yes".into());
    }

    if session.ssh.agent_forward {
        args.push("-o".into());
        args.push("ForwardAgent=yes".into());
    }

    if session.ssh.x11 {
        args.push("-X".into());
    }

    for extra in session
        .ssh
        .extra_args
        .iter()
        .map(|arg| arg.trim())
        .filter(|arg| !arg.is_empty())
    {
        args.push(extra.to_string());
    }

    args.push(session.host.clone());

    let mut env = vec![
        ("TERM".to_string(), "xterm-256color".to_string()),
        ("COLORTERM".to_string(), "truecolor".to_string()),
    ];

    if let Some(askpass) = askpass {
        env.push(("SSH_ASKPASS".into(), askpass.program.display().to_string()));
        env.push(("SSH_ASKPASS_REQUIRE".into(), "force".into()));
        env.push(("LT_ASKPASS_SESSION".into(), askpass.session_id.to_string()));
        env.push(("LT_ASKPASS_KIND".into(), askpass.kind.as_str().into()));
    }

    Ok(CommandSpec {
        program: std::env::var("LIKE_TERMINAL_SSH").unwrap_or_else(|_| "ssh".to_string()),
        args,
        env,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use lt_core::AuthMethod;
    use std::path::PathBuf;

    fn session() -> Session {
        let mut session = Session::new("m1", "10.0.0.21");
        session.user = "admin".into();
        session
    }

    fn args(session: &Session) -> Vec<String> {
        build(session, None).unwrap().args
    }

    #[test]
    fn password_session_falls_back_to_pubkey() {
        let args = args(&session());
        assert!(args.contains(&"-tt".to_string()));
        assert_eq!(args.last().unwrap(), "10.0.0.21");
        assert!(!args.iter().any(|arg| arg == "PubkeyAuthentication=no"));
        assert!(args
            .iter()
            .any(|arg| arg == "PreferredAuthentications=password,keyboard-interactive,publickey"));
        assert!(args
            .windows(2)
            .any(|pair| pair[0] == "-p" && pair[1] == "22"));
        assert!(args
            .windows(2)
            .any(|pair| pair[0] == "-l" && pair[1] == "admin"));
    }

    #[test]
    fn custom_port_is_forwarded() {
        let mut session = session();
        session.port = 2222;
        let args = args(&session);
        assert!(args
            .windows(2)
            .any(|pair| pair[0] == "-p" && pair[1] == "2222"));
    }

    #[test]
    fn public_key_requires_path() {
        let mut session = session();
        session.auth.method = AuthMethod::PublicKey;
        assert_eq!(build(&session, None), Err(BuildError::MissingKey));
    }

    #[test]
    fn public_key_adds_identity() {
        let mut session = session();
        session.auth.method = AuthMethod::PublicKey;
        session.auth.key_path = Some(PathBuf::from("/home/user/.ssh/id_ed25519"));
        let args = args(&session);
        assert!(args
            .windows(2)
            .any(|pair| pair[0] == "-i" && pair[1] == "/home/user/.ssh/id_ed25519"));
        assert!(args.iter().any(|arg| arg == "IdentitiesOnly=yes"));
    }

    #[test]
    fn jump_and_compression_are_added() {
        let mut session = session();
        session.ssh.jump = Some("bastion".into());
        session.ssh.compression = true;
        let args = args(&session);
        assert!(args
            .windows(2)
            .any(|pair| pair[0] == "-J" && pair[1] == "bastion"));
        assert!(args.iter().any(|arg| arg == "Compression=yes"));
    }

    #[test]
    fn askpass_env_is_present_when_requested() {
        let session = session();
        let spec = AskpassSpec {
            program: PathBuf::from("/usr/bin/like-terminal"),
            session_id: Uuid::new_v4(),
            kind: AskpassKind::Password,
        };
        let command = build(&session, Some(&spec)).unwrap();
        let env: std::collections::HashMap<_, _> = command.env.into_iter().collect();
        assert_eq!(env.get("SSH_ASKPASS").unwrap(), "/usr/bin/like-terminal");
        assert_eq!(env.get("SSH_ASKPASS_REQUIRE").unwrap(), "force");
        assert_eq!(env.get("LT_ASKPASS_KIND").unwrap(), "password");
    }

    #[test]
    fn empty_host_starts_a_local_shell() {
        let session = Session::new("local", "");
        let command = build(&session, None).expect("local shell");
        assert!(command.program.ends_with("sh"));
        assert!(command.args.is_empty());
    }

    #[test]
    fn hosts_starting_with_dash_are_rejected() {
        let mut session = session();
        session.host = "-oProxyCommand=evil".into();
        assert_eq!(build(&session, None), Err(BuildError::InvalidHost));
    }

    #[test]
    fn user_with_space_is_rejected() {
        let mut session = session();
        session.user = "ad min".into();
        assert_eq!(build(&session, None), Err(BuildError::InvalidUser));
    }
}
