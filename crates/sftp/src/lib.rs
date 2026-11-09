use std::path::{Path, PathBuf};
use std::sync::mpsc::Sender;
use std::sync::Arc;
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::sync::mpsc::{UnboundedReceiver, UnboundedSender};

const CONNECT_TIMEOUT: Duration = Duration::from_secs(15);
const SHUTDOWN_TIMEOUT: Duration = Duration::from_secs(5);
const BUFFER_SIZE: usize = 64 * 1024;

#[derive(Debug, thiserror::Error)]
pub enum SftpError {
    #[error("conexão recusada: {0}")]
    Connect(String),
    #[error("falha na autenticação: {0}")]
    Auth(String),
    #[error("chave inválida: {0}")]
    Key(String),
    #[error("erro no canal SFTP: {0}")]
    Channel(String),
    #[error("erro de I/O: {0}")]
    Io(#[from] std::io::Error),
    #[error("sessão encerrada")]
    Closed,
}

impl From<russh::Error> for SftpError {
    fn from(error: russh::Error) -> Self {
        SftpError::Connect(error.to_string())
    }
}

#[derive(Debug, Clone)]
pub enum AuthSpec {
    Password(String),
    Key {
        path: PathBuf,
        passphrase: Option<String>,
    },
    Agent,
}

#[derive(Debug, Clone)]
pub struct SftpConfig {
    pub host: String,
    pub port: u16,
    pub user: String,
    pub auth: AuthSpec,
}

#[derive(Debug, Clone)]
pub struct Entry {
    pub name: String,
    pub path: String,
    pub is_dir: bool,
    pub is_symlink: bool,
    pub size: u64,
    pub mode: u32,
    pub modified: Option<i64>,
}

type Reply<T> = Sender<Result<T, SftpError>>;

enum Command {
    Home(Reply<String>),
    List {
        path: String,
        reply: Reply<Vec<Entry>>,
    },
    CreateDir {
        path: String,
        reply: Reply<()>,
    },
    RemoveFile {
        path: String,
        reply: Reply<()>,
    },
    RemoveDir {
        path: String,
        reply: Reply<()>,
    },
    Rename {
        from: String,
        to: String,
        reply: Reply<()>,
    },
    Download {
        remote: String,
        local: PathBuf,
        reply: Reply<u64>,
    },
    Upload {
        local: PathBuf,
        remote: String,
        reply: Reply<u64>,
    },
    Close(Reply<()>),
}

pub struct SftpSession {
    command_tx: UnboundedSender<Command>,
    worker: Option<JoinHandle<()>>,
}

impl SftpSession {
    pub fn connect(config: SftpConfig) -> Result<Self, SftpError> {
        let (command_tx, command_rx) = tokio::sync::mpsc::unbounded_channel();
        let (ready_tx, ready_rx) = std::sync::mpsc::channel();
        let worker = std::thread::Builder::new()
            .name("lt-sftp".into())
            .spawn(move || run_worker(config, command_rx, ready_tx))
            .map_err(|error| {
                SftpError::Connect(format!("falha ao iniciar thread SFTP: {error}"))
            })?;

        match ready_rx.recv() {
            Ok(Ok(())) => Ok(Self {
                command_tx,
                worker: Some(worker),
            }),
            Ok(Err(error)) => Err(error),
            Err(_) => Err(SftpError::Connect(
                "thread SFTP encerrou durante a conexão".into(),
            )),
        }
    }

    pub fn home(&self) -> Result<String, SftpError> {
        self.request(Command::Home)
    }

    pub fn list(&self, path: &str) -> Result<Vec<Entry>, SftpError> {
        self.request(|reply| Command::List {
            path: path.to_string(),
            reply,
        })
    }

    pub fn create_dir(&self, path: &str) -> Result<(), SftpError> {
        self.request(|reply| Command::CreateDir {
            path: path.to_string(),
            reply,
        })
    }

    pub fn remove_file(&self, path: &str) -> Result<(), SftpError> {
        self.request(|reply| Command::RemoveFile {
            path: path.to_string(),
            reply,
        })
    }

    pub fn remove_dir(&self, path: &str) -> Result<(), SftpError> {
        self.request(|reply| Command::RemoveDir {
            path: path.to_string(),
            reply,
        })
    }

    pub fn rename(&self, from: &str, to: &str) -> Result<(), SftpError> {
        self.request(|reply| Command::Rename {
            from: from.to_string(),
            to: to.to_string(),
            reply,
        })
    }

    pub fn download(&self, remote: &str, local: &Path) -> Result<u64, SftpError> {
        self.request(|reply| Command::Download {
            remote: remote.to_string(),
            local: local.to_path_buf(),
            reply,
        })
    }

    pub fn upload(&self, local: &Path, remote: &str) -> Result<u64, SftpError> {
        self.request(|reply| Command::Upload {
            local: local.to_path_buf(),
            remote: remote.to_string(),
            reply,
        })
    }

    pub fn close(&self) {
        let (reply, response) = std::sync::mpsc::channel();
        if self.command_tx.send(Command::Close(reply)).is_ok() {
            let _ = response.recv_timeout(SHUTDOWN_TIMEOUT);
        }
    }

    fn request<T>(&self, build: impl FnOnce(Reply<T>) -> Command) -> Result<T, SftpError> {
        let (reply, response) = std::sync::mpsc::channel();
        self.command_tx
            .send(build(reply))
            .map_err(|_| SftpError::Closed)?;
        response.recv().map_err(|_| SftpError::Closed)?
    }
}

impl Drop for SftpSession {
    fn drop(&mut self) {
        self.close();
        if let Some(worker) = self.worker.take() {
            let deadline = Instant::now() + SHUTDOWN_TIMEOUT;
            while !worker.is_finished() && Instant::now() < deadline {
                std::thread::sleep(Duration::from_millis(10));
            }
            if worker.is_finished() {
                let _ = worker.join();
            }
        }
    }
}

struct ClientHandler {
    host: String,
    port: u16,
}

impl russh::client::Handler for ClientHandler {
    type Error = SftpError;

    async fn check_server_key(
        &mut self,
        server_public_key: &russh::keys::PublicKeyOrCertificate,
    ) -> Result<bool, Self::Error> {
        use russh::keys::{PublicKey, PublicKeyOrCertificate};

        let key = match server_public_key {
            PublicKeyOrCertificate::PublicKey { key, .. } => key.clone(),
            PublicKeyOrCertificate::Certificate(certificate) => {
                PublicKey::from(certificate.public_key().clone())
            }
        };

        match russh::keys::check_known_hosts(&self.host, self.port, &key) {
            Ok(true) => Ok(true),
            Ok(false) => {
                tracing::warn!(
                    host = %self.host,
                    port = self.port,
                    "chave do servidor ausente no known_hosts; aceitando conexão"
                );
                Ok(true)
            }
            Err(russh::keys::Error::KeyChanged { line }) => Err(SftpError::Connect(format!(
                "chave do servidor divergente do known_hosts (linha {line})"
            ))),
            Err(error) => {
                tracing::warn!(
                    host = %self.host,
                    port = self.port,
                    %error,
                    "não foi possível verificar o known_hosts; aceitando conexão"
                );
                Ok(true)
            }
        }
    }
}

fn run_worker(
    config: SftpConfig,
    mut commands: UnboundedReceiver<Command>,
    ready: Sender<Result<(), SftpError>>,
) {
    let runtime = match tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
    {
        Ok(runtime) => runtime,
        Err(error) => {
            let _ = ready.send(Err(SftpError::Connect(format!(
                "falha ao criar runtime Tokio: {error}"
            ))));
            return;
        }
    };

    runtime.block_on(async move {
        let (handle, sftp) = match tokio::time::timeout(CONNECT_TIMEOUT, establish(&config)).await {
            Ok(Ok(established)) => established,
            Ok(Err(error)) => {
                let _ = ready.send(Err(error));
                return;
            }
            Err(_) => {
                let _ = ready.send(Err(SftpError::Connect(format!(
                    "tempo esgotado ao conectar em {}:{}",
                    config.host, config.port
                ))));
                return;
            }
        };

        if ready.send(Ok(())).is_err() {
            return;
        }

        while let Some(command) = commands.recv().await {
            match command {
                Command::Home(reply) => {
                    let result = sftp.canonicalize(".").await.map_err(channel_error);
                    let _ = reply.send(result);
                }
                Command::List { path, reply } => {
                    let _ = reply.send(list_dir(&sftp, &path).await);
                }
                Command::CreateDir { path, reply } => {
                    let result = sftp.create_dir(path).await.map_err(channel_error);
                    let _ = reply.send(result);
                }
                Command::RemoveFile { path, reply } => {
                    let result = sftp.remove_file(path).await.map_err(channel_error);
                    let _ = reply.send(result);
                }
                Command::RemoveDir { path, reply } => {
                    let result = sftp.remove_dir(path).await.map_err(channel_error);
                    let _ = reply.send(result);
                }
                Command::Rename { from, to, reply } => {
                    let result = sftp.rename(from, to).await.map_err(channel_error);
                    let _ = reply.send(result);
                }
                Command::Download {
                    remote,
                    local,
                    reply,
                } => {
                    let _ = reply.send(download(&sftp, &remote, &local).await);
                }
                Command::Upload {
                    local,
                    remote,
                    reply,
                } => {
                    let _ = reply.send(upload(&sftp, &local, &remote).await);
                }
                Command::Close(reply) => {
                    let result = sftp.close().await.map_err(channel_error);
                    let _ = handle
                        .disconnect(russh::Disconnect::ByApplication, "", "")
                        .await;
                    let _ = reply.send(result);
                    break;
                }
            }
        }
    });
}

async fn establish(
    config: &SftpConfig,
) -> Result<
    (
        russh::client::Handle<ClientHandler>,
        russh_sftp::client::SftpSession,
    ),
    SftpError,
> {
    let handler = ClientHandler {
        host: config.host.clone(),
        port: config.port,
    };
    let mut handle = russh::client::connect(
        Arc::new(russh::client::Config::default()),
        (config.host.as_str(), config.port),
        handler,
    )
    .await?;

    authenticate(&mut handle, config).await?;

    let channel = handle
        .channel_open_session()
        .await
        .map_err(|error| SftpError::Channel(error.to_string()))?;
    channel
        .request_subsystem(true, "sftp")
        .await
        .map_err(|error| SftpError::Channel(error.to_string()))?;
    let sftp = russh_sftp::client::SftpSession::new(channel.into_stream())
        .await
        .map_err(channel_error)?;

    Ok((handle, sftp))
}

async fn authenticate(
    handle: &mut russh::client::Handle<ClientHandler>,
    config: &SftpConfig,
) -> Result<(), SftpError> {
    match &config.auth {
        AuthSpec::Password(password) => {
            let result = handle
                .authenticate_password(&config.user, password)
                .await
                .map_err(|error| SftpError::Auth(error.to_string()))?;
            if result.success() {
                Ok(())
            } else {
                Err(SftpError::Auth("senha recusada pelo servidor".into()))
            }
        }
        AuthSpec::Key { path, passphrase } => {
            let key = russh::keys::load_secret_key(path, passphrase.as_deref())
                .map_err(|error| SftpError::Key(error.to_string()))?;
            let hash_alg = handle
                .best_supported_rsa_hash()
                .await
                .map_err(|error| SftpError::Auth(error.to_string()))?
                .flatten();
            let key = russh::keys::PrivateKeyWithHashAlg::new(Arc::new(key), hash_alg);
            let result = handle
                .authenticate_publickey(&config.user, key)
                .await
                .map_err(|error| SftpError::Auth(error.to_string()))?;
            if result.success() {
                Ok(())
            } else {
                Err(SftpError::Auth("chave recusada pelo servidor".into()))
            }
        }
        AuthSpec::Agent => authenticate_agent(handle, &config.user).await,
    }
}

async fn authenticate_agent(
    handle: &mut russh::client::Handle<ClientHandler>,
    user: &str,
) -> Result<(), SftpError> {
    let mut agent = russh::keys::agent::client::AgentClient::connect_env()
        .await
        .map_err(|error| SftpError::Auth(format!("agente SSH indisponível: {error}")))?;
    let identities = agent
        .request_identities()
        .await
        .map_err(|error| SftpError::Auth(format!("falha ao listar identidades: {error}")))?;
    if identities.is_empty() {
        return Err(SftpError::Auth("agente SSH não possui identidades".into()));
    }

    let hash_alg = handle
        .best_supported_rsa_hash()
        .await
        .map_err(|error| SftpError::Auth(error.to_string()))?
        .flatten();

    for identity in identities {
        let key = identity.public_key().into_owned();
        let identity_hash_alg = if key.algorithm().is_rsa() {
            hash_alg
        } else {
            None
        };
        match handle
            .authenticate_publickey_with(user, key, identity_hash_alg, &mut agent)
            .await
        {
            Ok(result) if result.success() => return Ok(()),
            Ok(_) => continue,
            Err(error) => tracing::warn!(%error, "identidade do agente recusada"),
        }
    }

    Err(SftpError::Auth(
        "nenhuma identidade do agente foi aceita".into(),
    ))
}

async fn list_dir(
    sftp: &russh_sftp::client::SftpSession,
    path: &str,
) -> Result<Vec<Entry>, SftpError> {
    let mut entries = Vec::new();
    for dir_entry in sftp.read_dir(path).await.map_err(channel_error)? {
        let name = dir_entry.file_name();
        let metadata = dir_entry.metadata();
        entries.push(Entry {
            path: join_remote(path, &name),
            name,
            is_dir: metadata.is_dir(),
            is_symlink: metadata.is_symlink(),
            size: metadata.size.unwrap_or(0),
            mode: metadata.permissions.unwrap_or(0),
            modified: metadata.mtime.map(i64::from),
        });
    }

    entries.sort_by(|left, right| {
        right
            .is_dir
            .cmp(&left.is_dir)
            .then_with(|| left.name.to_lowercase().cmp(&right.name.to_lowercase()))
            .then_with(|| left.name.cmp(&right.name))
    });

    Ok(entries)
}

async fn download(
    sftp: &russh_sftp::client::SftpSession,
    remote: &str,
    local: &Path,
) -> Result<u64, SftpError> {
    let mut remote_file = sftp.open(remote).await.map_err(channel_error)?;
    if let Some(parent) = local.parent() {
        if !parent.as_os_str().is_empty() {
            tokio::fs::create_dir_all(parent).await?;
        }
    }
    let mut local_file = tokio::fs::File::create(local).await?;
    let mut buffer = vec![0u8; BUFFER_SIZE];
    let mut total = 0u64;

    loop {
        let read = remote_file.read(&mut buffer).await?;
        if read == 0 {
            break;
        }
        local_file.write_all(&buffer[..read]).await?;
        total += read as u64;
    }

    local_file.flush().await?;
    remote_file
        .close()
        .await
        .map_err(|error| SftpError::Channel(error.to_string()))?;

    Ok(total)
}

async fn upload(
    sftp: &russh_sftp::client::SftpSession,
    local: &Path,
    remote: &str,
) -> Result<u64, SftpError> {
    let mut local_file = tokio::fs::File::open(local).await?;
    let mut remote_file = sftp.create(remote).await.map_err(channel_error)?;
    let mut buffer = vec![0u8; BUFFER_SIZE];
    let mut total = 0u64;

    loop {
        let read = local_file.read(&mut buffer).await?;
        if read == 0 {
            break;
        }
        remote_file.write_all(&buffer[..read]).await?;
        total += read as u64;
    }

    remote_file
        .close()
        .await
        .map_err(|error| SftpError::Channel(error.to_string()))?;

    Ok(total)
}

fn channel_error(error: russh_sftp::client::error::Error) -> SftpError {
    SftpError::Channel(error.to_string())
}

fn join_remote(base: &str, name: &str) -> String {
    if base.is_empty() {
        name.to_string()
    } else if base.ends_with('/') {
        format!("{base}{name}")
    } else {
        format!("{base}/{name}")
    }
}

#[cfg(test)]
mod tests {
    use super::join_remote;

    #[test]
    fn joins_absolute_base() {
        assert_eq!(
            join_remote("/home/user", "arquivo.txt"),
            "/home/user/arquivo.txt"
        );
    }

    #[test]
    fn joins_root_base() {
        assert_eq!(join_remote("/", "arquivo.txt"), "/arquivo.txt");
    }

    #[test]
    fn keeps_single_separator() {
        assert_eq!(
            join_remote("/home/user/", "arquivo.txt"),
            "/home/user/arquivo.txt"
        );
    }

    #[test]
    fn handles_empty_base() {
        assert_eq!(join_remote("", "arquivo.txt"), "arquivo.txt");
    }
}
