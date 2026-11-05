use std::io;
use std::net::{TcpStream, ToSocketAddrs};
use std::time::Duration;

#[derive(Debug, thiserror::Error)]
pub enum ProbeError {
    #[error("não foi possível resolver {host}: {source}")]
    Resolve { host: String, source: io::Error },
    #[error("{host}:{port} recusou a conexão")]
    Refused { host: String, port: u16 },
    #[error("tempo esgotado ao conectar em {host}:{port}")]
    Timeout { host: String, port: u16 },
    #[error("falha ao conectar em {host}:{port}: {source}")]
    Io {
        host: String,
        port: u16,
        source: io::Error,
    },
}

pub fn probe(host: &str, port: u16, timeout: Duration) -> Result<(), ProbeError> {
    let addresses = (host, port)
        .to_socket_addrs()
        .map_err(|source| ProbeError::Resolve {
            host: host.to_string(),
            source,
        })?;

    let mut last_error = None;
    for address in addresses {
        match TcpStream::connect_timeout(&address, timeout) {
            Ok(_) => return Ok(()),
            Err(source) if source.kind() == io::ErrorKind::ConnectionRefused => {
                last_error = Some(ProbeError::Refused {
                    host: host.to_string(),
                    port,
                });
            }
            Err(source) if source.kind() == io::ErrorKind::TimedOut => {
                last_error = Some(ProbeError::Timeout {
                    host: host.to_string(),
                    port,
                });
            }
            Err(source) => {
                last_error = Some(ProbeError::Io {
                    host: host.to_string(),
                    port,
                    source,
                });
            }
        }
    }

    Err(last_error.unwrap_or(ProbeError::Resolve {
        host: host.to_string(),
        source: io::Error::new(io::ErrorKind::NotFound, "nenhum endereço encontrado"),
    }))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::net::TcpListener;

    #[test]
    fn probe_succeeds_against_open_port() {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let port = listener.local_addr().unwrap().port();
        assert!(probe("127.0.0.1", port, Duration::from_secs(1)).is_ok());
    }

    #[test]
    fn probe_fails_on_closed_port() {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let port = listener.local_addr().unwrap().port();
        drop(listener);
        assert!(probe("127.0.0.1", port, Duration::from_millis(500)).is_err());
    }
}
