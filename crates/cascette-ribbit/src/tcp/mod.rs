//! TCP Ribbit protocol implementation.

use crate::error::{ProtocolError, ServerError};
use crate::server::AppState;
use std::net::SocketAddr;
use std::sync::Arc;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::net::{TcpListener, TcpStream};
use tokio::time::{Duration, timeout};

pub mod handlers;
pub mod v1;
pub mod v2;

/// Start TCP server for Ribbit v1/v2 protocols.
///
/// # Errors
///
/// Returns `ServerError` if the server fails to bind or encounters a fatal error.
pub async fn start_server(bind_addr: SocketAddr, state: Arc<AppState>) -> Result<(), ServerError> {
    let listener =
        TcpListener::bind(bind_addr)
            .await
            .map_err(|source| ServerError::TcpBindFailed {
                addr: bind_addr,
                source,
            })?;

    tracing::info!("TCP server listening on {bind_addr}");

    loop {
        let (socket, addr) = listener
            .accept()
            .await
            .map_err(|e| ServerError::Shutdown(format!("Failed to accept TCP connection: {e}")))?;

        let state = state.clone();

        // Spawn a task for each connection
        tokio::spawn(async move {
            if let Err(e) = handle_connection(socket, state).await {
                tracing::warn!("TCP connection from {addr} failed: {e}");
            }
        });
    }
}

/// Handle a single TCP connection.
///
/// # Errors
///
/// Returns `ProtocolError` if connection handling fails.
async fn handle_connection(
    mut socket: TcpStream,
    state: Arc<AppState>,
) -> Result<(), ProtocolError> {
    let addr = socket.peer_addr()?;
    tracing::debug!("Accepted TCP connection from {addr}");

    // Read command with timeout
    let mut reader = BufReader::new(&mut socket);
    let mut command = String::new();

    let read_result = timeout(Duration::from_secs(10), reader.read_line(&mut command)).await;

    match read_result {
        Ok(Ok(0)) => {
            tracing::debug!("TCP connection closed by client: {addr}");
            return Ok(());
        }
        Ok(Ok(_)) => {
            // Command received, process it
            let command = command.trim();
            tracing::debug!("Received TCP command from {addr}: {command}");

            let response = handlers::handle_command(command, &state)?;

            // Write response
            socket.write_all(response.as_bytes()).await?;
            socket.flush().await?;

            tracing::debug!("Sent TCP response to {addr}: {} bytes", response.len());
        }
        Ok(Err(e)) => {
            return Err(ProtocolError::Io(e));
        }
        Err(_) => {
            return Err(ProtocolError::Timeout { seconds: 10 });
        }
    }

    // Close connection immediately after response
    socket.shutdown().await?;
    Ok(())
}

#[cfg(test)]
mod tests {
    #[test]
    fn test_tcp_module_exists() {
        // Module compiles and is accessible - test passes
    }
}
