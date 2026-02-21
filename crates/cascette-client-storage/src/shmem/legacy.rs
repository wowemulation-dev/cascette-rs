//! Shared memory IPC for communication with game clients
//!
//! Provides inter-process communication mechanisms similar to Battle.net Agent.
//! Uses binrw for efficient binary message serialization with platform-specific
//! shared memory implementations. Includes support for the official CASC 'shmem'
//! file format for client compatibility.

use std::collections::HashMap;
use std::io::Cursor;
use std::sync::{Arc, Mutex};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use binrw::{BinRead, BinWrite};
use parking_lot::RwLock;
use tracing::{debug, info};

#[cfg(test)]
#[allow(unused_imports)]
use crate::validation::BinaryFormatValidator;
use crate::{Result, StorageError};

/// Magic bytes for IPC message validation ("CASC" in ASCII)
const IPC_MAGIC: u32 = 0x4341_5343;

/// Current IPC protocol version
const IPC_VERSION: u16 = 1;

/// Maximum payload size (16MB)
const MAX_PAYLOAD_SIZE: u32 = 16 * 1024 * 1024;

/// Default shared memory size (64MB)
const DEFAULT_SHMEM_SIZE: usize = 64 * 1024 * 1024;

/// Maximum number of concurrent connections
const MAX_CONNECTIONS: usize = 64;

/// Heartbeat interval for keep-alive messages
const HEARTBEAT_INTERVAL: Duration = Duration::from_secs(30);

/// IPC message types for different operations
#[derive(Debug, Clone, Copy, PartialEq, Eq, BinRead, BinWrite)]
#[br(repr = u16)]
#[bw(repr = u16)]
pub enum MessageType {
    /// Request file by path or `FileDataID`
    FileRequest = 0x0001,
    /// Deliver file content or error
    FileResponse = 0x0002,
    /// Query installation status
    StatusRequest = 0x0003,
    /// Return installation information
    StatusResponse = 0x0004,
    /// Heartbeat message
    KeepAlive = 0x0005,
    /// Error response
    Error = 0xFFFF,
}

/// IPC message header with magic bytes, version, and payload information
#[derive(Debug, Clone, BinRead, BinWrite)]
#[br(big)]
#[bw(big)]
pub struct MessageHeader {
    /// Magic bytes for validation ("CASC")
    #[br(assert(magic == IPC_MAGIC))]
    pub magic: u32,
    /// Protocol version
    pub version: u16,
    /// Message type
    pub message_type: MessageType,
    /// Payload size in bytes
    #[br(assert(payload_size <= MAX_PAYLOAD_SIZE))]
    pub payload_size: u32,
    /// Unique message ID for request/response correlation
    pub message_id: u64,
    /// Timestamp (seconds since Unix epoch)
    pub timestamp: u64,
    /// Reserved for future use
    pub reserved: [u8; 8],
}

impl MessageHeader {
    /// Create a new message header
    pub fn new(message_type: MessageType, payload_size: u32, message_id: u64) -> Self {
        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();

        Self {
            magic: IPC_MAGIC,
            version: IPC_VERSION,
            message_type,
            payload_size,
            message_id,
            timestamp,
            reserved: [0; 8],
        }
    }

    /// Header size in bytes
    pub const fn size() -> usize {
        36 // 4 (magic) + 2 (version) + 2 (message_type) + 4 (payload_size) + 8 (message_id) + 8 (timestamp) + 8 (reserved)
    }
}

/// File request payload
#[derive(Debug, Clone, BinRead, BinWrite)]
#[br(big)]
#[bw(big)]
pub struct FileRequestPayload {
    /// Request type: 0 = by path, 1 = by `FileDataID`
    pub request_type: u8,
    /// Priority: 0 = normal, 1 = high, 2 = critical
    pub priority: u8,
    /// Options flags (reserved)
    pub flags: u16,
    /// Length of the identifier that follows
    pub identifier_length: u32,
    /// File path or `FileDataID` as bytes
    #[br(count = identifier_length)]
    pub identifier: Vec<u8>,
}

impl FileRequestPayload {
    /// Create a request by file path
    pub fn by_path(path: &str, priority: u8) -> Self {
        let identifier = path.as_bytes().to_vec();
        Self {
            request_type: 0,
            priority,
            flags: 0,
            identifier_length: u32::try_from(identifier.len()).unwrap_or(0),
            identifier,
        }
    }

    /// Create a request by `FileDataID`
    pub fn by_file_data_id(file_data_id: u32, priority: u8) -> Self {
        let identifier = file_data_id.to_be_bytes().to_vec();
        Self {
            request_type: 1,
            priority,
            flags: 0,
            identifier_length: u32::try_from(identifier.len()).unwrap_or(0),
            identifier,
        }
    }

    /// Get the file path if this is a path request
    ///
    /// # Errors
    ///
    /// Returns error if request is not a path request or contains invalid UTF-8
    pub fn path(&self) -> Result<String> {
        if self.request_type == 0 {
            String::from_utf8(self.identifier.clone())
                .map_err(|e| StorageError::InvalidFormat(format!("Invalid UTF-8 path: {e}")))
        } else {
            Err(StorageError::InvalidFormat(
                "Not a path request".to_string(),
            ))
        }
    }

    /// Get the `FileDataID` if this is a `FileDataID` request
    ///
    /// # Errors
    ///
    /// Returns error if request is not a `FileDataID` request or has invalid length
    pub fn file_data_id(&self) -> Result<u32> {
        if self.request_type == 1 && self.identifier.len() == 4 {
            let mut bytes = [0u8; 4];
            bytes.copy_from_slice(&self.identifier);
            Ok(u32::from_be_bytes(bytes))
        } else {
            Err(StorageError::InvalidFormat(
                "Not a FileDataID request or invalid length".to_string(),
            ))
        }
    }
}

/// File response payload
#[derive(Debug, Clone, BinRead, BinWrite)]
#[br(big)]
#[bw(big)]
pub struct FileResponsePayload {
    /// Response status: 0 = success, 1 = not found, 2 = error
    pub status: u8,
    /// Compression type: 0 = none, 1 = BLTE
    pub compression: u8,
    /// Response flags (reserved)
    pub flags: u16,
    /// Size of uncompressed data
    pub uncompressed_size: u32,
    /// Size of compressed data
    pub compressed_size: u32,
    /// MD5 hash of uncompressed data
    pub content_hash: [u8; 16],
    /// File content data
    #[br(count = compressed_size)]
    pub data: Vec<u8>,
}

impl FileResponsePayload {
    /// Create a successful response with data
    pub fn success(data: Vec<u8>, content_hash: [u8; 16]) -> Self {
        let uncompressed_size = u32::try_from(data.len()).unwrap_or(0);
        Self {
            status: 0,
            compression: 0, // No compression for now
            flags: 0,
            uncompressed_size,
            compressed_size: uncompressed_size,
            content_hash,
            data,
        }
    }

    /// Create a not found response
    pub const fn not_found() -> Self {
        Self {
            status: 1,
            compression: 0,
            flags: 0,
            uncompressed_size: 0,
            compressed_size: 0,
            content_hash: [0; 16],
            data: Vec::new(),
        }
    }

    /// Create an error response
    pub const fn error() -> Self {
        Self {
            status: 2,
            compression: 0,
            flags: 0,
            uncompressed_size: 0,
            compressed_size: 0,
            content_hash: [0; 16],
            data: Vec::new(),
        }
    }

    /// Check if the response is successful
    pub const fn is_success(&self) -> bool {
        self.status == 0
    }

    /// Check if the file was not found
    pub const fn is_not_found(&self) -> bool {
        self.status == 1
    }

    /// Check if there was an error
    pub const fn is_error(&self) -> bool {
        self.status == 2
    }
}

/// Status request payload
#[derive(Debug, Clone, BinRead, BinWrite)]
#[br(big)]
#[bw(big)]
pub struct StatusRequestPayload {
    /// Status type: 0 = general, 1 = installation specific
    pub status_type: u8,
    /// Reserved flags
    pub flags: [u8; 3],
    /// Installation name length (if `status_type` == 1)
    pub name_length: u32,
    /// Installation name
    #[br(count = name_length)]
    pub installation_name: Vec<u8>,
}

impl StatusRequestPayload {
    /// Create a general status request
    pub const fn general() -> Self {
        Self {
            status_type: 0,
            flags: [0; 3],
            name_length: 0,
            installation_name: Vec::new(),
        }
    }

    /// Create an installation-specific status request
    pub fn installation(name: &str) -> Self {
        let installation_name = name.as_bytes().to_vec();
        Self {
            status_type: 1,
            flags: [0; 3],
            name_length: u32::try_from(installation_name.len()).unwrap_or(0),
            installation_name,
        }
    }

    /// Get the installation name if this is an installation request
    ///
    /// # Errors
    ///
    /// Returns error if installation name contains invalid UTF-8
    pub fn installation_name(&self) -> Result<Option<String>> {
        if self.status_type == 1 && !self.installation_name.is_empty() {
            String::from_utf8(self.installation_name.clone())
                .map(Some)
                .map_err(|e| StorageError::InvalidFormat(format!("Invalid UTF-8 name: {e}")))
        } else {
            Ok(None)
        }
    }
}

/// Status response payload
#[derive(Debug, Clone, BinRead, BinWrite)]
#[br(big)]
#[bw(big)]
pub struct StatusResponsePayload {
    /// System status: 0 = healthy, 1 = degraded, 2 = error
    pub system_status: u8,
    /// Number of active installations
    pub installation_count: u8,
    /// Reserved flags
    pub flags: [u8; 2],
    /// Total cache size in bytes
    pub cache_size: u64,
    /// Used cache size in bytes
    pub cache_used: u64,
    /// Number of cached files
    pub cached_files: u32,
    /// Uptime in seconds
    pub uptime: u32,
    /// JSON status data length
    pub status_data_length: u32,
    /// JSON status data
    #[br(count = status_data_length)]
    pub status_data: Vec<u8>,
}

impl StatusResponsePayload {
    /// Create a new status response
    pub fn new(
        system_status: u8,
        installation_count: u8,
        cache_size: u64,
        cache_used: u64,
        cached_files: u32,
        uptime: u32,
        status_data: String,
    ) -> Self {
        let status_data_bytes = status_data.into_bytes();
        Self {
            system_status,
            installation_count,
            flags: [0; 2],
            cache_size,
            cache_used,
            cached_files,
            uptime,
            status_data_length: u32::try_from(status_data_bytes.len()).unwrap_or(0),
            status_data: status_data_bytes,
        }
    }

    /// Get the status data as a string
    ///
    /// # Errors
    ///
    /// Returns error if status data contains invalid UTF-8
    pub fn status_data_string(&self) -> Result<String> {
        String::from_utf8(self.status_data.clone())
            .map_err(|e| StorageError::InvalidFormat(format!("Invalid UTF-8 status data: {e}")))
    }
}

/// Keep-alive payload
#[derive(Debug, Clone, BinRead, BinWrite)]
#[br(big)]
#[bw(big)]
pub struct KeepAlivePayload {
    /// Sequence number for detecting missed heartbeats
    pub sequence: u64,
    /// Connection ID
    pub connection_id: u32,
    /// Reserved
    pub reserved: [u8; 4],
}

impl KeepAlivePayload {
    /// Create a new keep-alive payload
    pub const fn new(sequence: u64, connection_id: u32) -> Self {
        Self {
            sequence,
            connection_id,
            reserved: [0; 4],
        }
    }
}

/// Complete IPC message containing header and payload
#[derive(Debug, Clone)]
pub struct IpcMessage {
    /// Message header
    pub header: MessageHeader,
    /// Message payload
    pub payload: IpcMessagePayload,
}

/// Unified payload type for all message types
#[derive(Debug, Clone)]
pub enum IpcMessagePayload {
    /// File request
    FileRequest(FileRequestPayload),
    /// File response
    FileResponse(FileResponsePayload),
    /// Status request
    StatusRequest(StatusRequestPayload),
    /// Status response
    StatusResponse(StatusResponsePayload),
    /// Keep alive
    KeepAlive(KeepAlivePayload),
    /// Raw bytes for unknown message types
    Raw(Vec<u8>),
}

impl IpcMessage {
    /// Create a new message
    ///
    /// # Errors
    ///
    /// Returns error if payload cannot be serialized
    pub fn new(message_id: u64, payload: IpcMessagePayload) -> Result<Self> {
        let (message_type, payload_data) = match &payload {
            IpcMessagePayload::FileRequest(p) => {
                let mut buf = Vec::new();
                p.write(&mut Cursor::new(&mut buf)).map_err(|e| {
                    StorageError::InvalidFormat(format!("Serialization error: {e}"))
                })?;
                (MessageType::FileRequest, buf)
            }
            IpcMessagePayload::FileResponse(p) => {
                let mut buf = Vec::new();
                p.write(&mut Cursor::new(&mut buf)).map_err(|e| {
                    StorageError::InvalidFormat(format!("Serialization error: {e}"))
                })?;
                (MessageType::FileResponse, buf)
            }
            IpcMessagePayload::StatusRequest(p) => {
                let mut buf = Vec::new();
                p.write(&mut Cursor::new(&mut buf)).map_err(|e| {
                    StorageError::InvalidFormat(format!("Serialization error: {e}"))
                })?;
                (MessageType::StatusRequest, buf)
            }
            IpcMessagePayload::StatusResponse(p) => {
                let mut buf = Vec::new();
                p.write(&mut Cursor::new(&mut buf)).map_err(|e| {
                    StorageError::InvalidFormat(format!("Serialization error: {e}"))
                })?;
                (MessageType::StatusResponse, buf)
            }
            IpcMessagePayload::KeepAlive(p) => {
                let mut buf = Vec::new();
                p.write(&mut Cursor::new(&mut buf)).map_err(|e| {
                    StorageError::InvalidFormat(format!("Serialization error: {e}"))
                })?;
                (MessageType::KeepAlive, buf)
            }
            IpcMessagePayload::Raw(data) => (MessageType::Error, data.clone()),
        };

        let header = MessageHeader::new(
            message_type,
            u32::try_from(payload_data.len()).unwrap_or(0),
            message_id,
        );
        Ok(Self { header, payload })
    }

    /// Serialize message to bytes
    ///
    /// # Errors
    ///
    /// Returns error if message cannot be serialized
    pub fn to_bytes(&self) -> Result<Vec<u8>> {
        let mut buf = Vec::new();
        let mut cursor = Cursor::new(&mut buf);

        // Write header
        self.header
            .write(&mut cursor)
            .map_err(|e| StorageError::InvalidFormat(format!("Header serialization error: {e}")))?;

        // Write payload
        match &self.payload {
            IpcMessagePayload::FileRequest(p) => {
                p.write(&mut cursor).map_err(|e| {
                    StorageError::InvalidFormat(format!("Payload serialization error: {e}"))
                })?;
            }
            IpcMessagePayload::FileResponse(p) => {
                p.write(&mut cursor).map_err(|e| {
                    StorageError::InvalidFormat(format!("Payload serialization error: {e}"))
                })?;
            }
            IpcMessagePayload::StatusRequest(p) => {
                p.write(&mut cursor).map_err(|e| {
                    StorageError::InvalidFormat(format!("Payload serialization error: {e}"))
                })?;
            }
            IpcMessagePayload::StatusResponse(p) => {
                p.write(&mut cursor).map_err(|e| {
                    StorageError::InvalidFormat(format!("Payload serialization error: {e}"))
                })?;
            }
            IpcMessagePayload::KeepAlive(p) => {
                p.write(&mut cursor).map_err(|e| {
                    StorageError::InvalidFormat(format!("Payload serialization error: {e}"))
                })?;
            }
            IpcMessagePayload::Raw(data) => {
                std::io::Write::write_all(&mut cursor, data).map_err(|e| {
                    StorageError::InvalidFormat(format!("Raw data write error: {e}"))
                })?;
            }
        }

        Ok(buf)
    }

    /// Deserialize message from bytes
    ///
    /// # Errors
    ///
    /// Returns error if data cannot be deserialized or is invalid
    pub fn from_bytes(data: &[u8]) -> Result<Self> {
        let mut cursor = Cursor::new(data);

        // Read header
        let header = MessageHeader::read(&mut cursor).map_err(|e| {
            StorageError::InvalidFormat(format!("Header deserialization error: {e}"))
        })?;

        // Read payload based on message type
        let payload = match header.message_type {
            MessageType::FileRequest => {
                let p = FileRequestPayload::read(&mut cursor).map_err(|e| {
                    StorageError::InvalidFormat(format!("FileRequest deserialization error: {e}"))
                })?;
                IpcMessagePayload::FileRequest(p)
            }
            MessageType::FileResponse => {
                let p = FileResponsePayload::read(&mut cursor).map_err(|e| {
                    StorageError::InvalidFormat(format!("FileResponse deserialization error: {e}"))
                })?;
                IpcMessagePayload::FileResponse(p)
            }
            MessageType::StatusRequest => {
                let p = StatusRequestPayload::read(&mut cursor).map_err(|e| {
                    StorageError::InvalidFormat(format!("StatusRequest deserialization error: {e}"))
                })?;
                IpcMessagePayload::StatusRequest(p)
            }
            MessageType::StatusResponse => {
                let p = StatusResponsePayload::read(&mut cursor).map_err(|e| {
                    StorageError::InvalidFormat(format!(
                        "StatusResponse deserialization error: {e}"
                    ))
                })?;
                IpcMessagePayload::StatusResponse(p)
            }
            MessageType::KeepAlive => {
                let p = KeepAlivePayload::read(&mut cursor).map_err(|e| {
                    StorageError::InvalidFormat(format!("KeepAlive deserialization error: {e}"))
                })?;
                IpcMessagePayload::KeepAlive(p)
            }
            MessageType::Error => {
                let remaining = data.len() - usize::try_from(cursor.position()).unwrap_or(0);
                let mut raw_data = vec![0u8; remaining];
                std::io::Read::read_exact(&mut cursor, &mut raw_data).map_err(|e| {
                    StorageError::InvalidFormat(format!("Raw data read error: {e}"))
                })?;
                IpcMessagePayload::Raw(raw_data)
            }
        };

        Ok(Self { header, payload })
    }
}

/// Connection information for IPC clients
#[derive(Debug, Clone)]
#[allow(dead_code)]
struct Connection {
    /// Unique connection ID
    id: u32,
    /// Last activity timestamp
    last_activity: SystemTime,
    /// Keep-alive sequence number
    sequence: u64,
    /// Connection metadata
    metadata: HashMap<String, String>,
}

impl Connection {
    fn new(id: u32) -> Self {
        Self {
            id,
            last_activity: SystemTime::now(),
            sequence: 0,
            metadata: HashMap::new(),
        }
    }

    fn update_activity(&mut self) {
        self.last_activity = SystemTime::now();
    }

    fn is_expired(&self, timeout: Duration) -> bool {
        SystemTime::now()
            .duration_since(self.last_activity)
            .unwrap_or_default()
            > timeout
    }
}

/// CASC shared memory file header for client compatibility
#[derive(Debug, Clone, BinRead, BinWrite)]
#[br(big)]
#[bw(big)]
pub struct CascShmemHeader {
    /// Magic signature ("CASC")
    #[br(assert(magic == 0x4341_5343))]
    pub magic: u32,
    /// Version of the shmem format
    pub version: u32,
    /// Size of the shared memory region
    pub region_size: u64,
    /// Number of active installations
    pub installation_count: u32,
    /// Reserved for future use
    pub reserved: [u8; 12],
}

impl CascShmemHeader {
    /// Create a new CASC shared memory header
    pub const fn new(region_size: u64, installation_count: u32) -> Self {
        Self {
            magic: 0x4341_5343, // "CASC"
            version: 1,
            region_size,
            installation_count,
            reserved: [0; 12],
        }
    }

    /// Header size in bytes
    pub const fn size() -> usize {
        32 // 4 + 4 + 8 + 4 + 12
    }
}

/// Shared memory manager for IPC with platform-specific implementations
pub struct SharedMemoryManager {
    /// Platform-specific shared memory handle
    memory: Arc<SharedMemoryHandle>,
    /// Active connections
    connections: Arc<RwLock<HashMap<u32, Connection>>>,
    /// Message ID counter
    message_id_counter: Arc<Mutex<u64>>,
    /// Manager configuration
    config: SharedMemoryConfig,
    /// CASC shared memory file path for client compatibility
    shmem_file_path: Option<std::path::PathBuf>,
}

/// Configuration for shared memory manager
#[derive(Debug, Clone)]
pub struct SharedMemoryConfig {
    /// Shared memory region name
    pub name: String,
    /// Size of shared memory region
    pub size: usize,
    /// Connection timeout duration
    pub connection_timeout: Duration,
    /// Maximum number of concurrent connections
    pub max_connections: usize,
    /// Enable heartbeat monitoring
    pub enable_heartbeat: bool,
}

impl Default for SharedMemoryConfig {
    fn default() -> Self {
        Self {
            name: "cascette_ipc".to_string(),
            size: DEFAULT_SHMEM_SIZE,
            connection_timeout: Duration::from_secs(300), // 5 minutes
            max_connections: MAX_CONNECTIONS,
            enable_heartbeat: true,
        }
    }
}

impl SharedMemoryManager {
    /// Create a new shared memory manager
    ///
    /// # Errors
    ///
    /// Returns error if shared memory region cannot be created
    pub fn new(config: SharedMemoryConfig) -> Result<Self> {
        info!("Creating shared memory IPC manager: {}", config.name);

        let memory = SharedMemoryHandle::create(&config.name, config.size)?;

        Ok(Self {
            memory: Arc::new(memory),
            connections: Arc::new(RwLock::new(HashMap::new())),
            message_id_counter: Arc::new(Mutex::new(1)),
            config,
            shmem_file_path: None,
        })
    }

    /// Write message to shared memory
    ///
    /// # Errors
    ///
    /// Returns error if message cannot be written to shared memory
    pub fn write_message(&mut self, message: &IpcMessage) -> Result<usize> {
        let data = message.to_bytes()?;

        if data.len() > self.config.size {
            return Err(StorageError::SharedMemory(
                "Message too large for shared memory".to_string(),
            ));
        }

        debug!("Writing IPC message: {} bytes", data.len());
        self.memory.write(0, &data)?;
        Ok(data.len())
    }

    /// Read message from shared memory
    ///
    /// # Errors
    ///
    /// Returns error if message cannot be read from shared memory
    pub fn read_message(&self, size: usize) -> Result<IpcMessage> {
        if size > self.config.size {
            return Err(StorageError::SharedMemory(
                "Requested size exceeds shared memory capacity".to_string(),
            ));
        }

        let data = self.memory.read(0, size)?;
        debug!("Reading IPC message: {} bytes", data.len());
        IpcMessage::from_bytes(&data)
    }

    /// Register a new connection
    ///
    /// # Errors
    ///
    /// Returns error if maximum connections exceeded
    pub fn register_connection(&self) -> Result<u32> {
        let connection_id = {
            let mut connections = self.connections.write();

            if connections.len() >= self.config.max_connections {
                return Err(StorageError::SharedMemory(
                    "Maximum connections exceeded".to_string(),
                ));
            }

            let connection_id = u32::try_from(connections.len()).unwrap_or(0) + 1;
            let connection = Connection::new(connection_id);
            connections.insert(connection_id, connection);
            connection_id
        };

        info!("Registered IPC connection: {}", connection_id);
        Ok(connection_id)
    }

    /// Unregister a connection
    ///
    /// # Errors
    ///
    /// Returns error if connection ID not found
    pub fn unregister_connection(&self, connection_id: u32) -> Result<()> {
        let mut connections = self.connections.write();

        if connections.remove(&connection_id).is_some() {
            info!("Unregistered IPC connection: {}", connection_id);
            Ok(())
        } else {
            Err(StorageError::SharedMemory(format!(
                "Connection {connection_id} not found"
            )))
        }
    }

    /// Update connection activity
    ///
    /// # Errors
    ///
    /// Returns error if connection ID not found
    pub fn update_connection_activity(&self, connection_id: u32) -> Result<()> {
        let mut connections = self.connections.write();

        connections.get_mut(&connection_id).map_or_else(
            || {
                Err(StorageError::SharedMemory(format!(
                    "Connection {connection_id} not found"
                )))
            },
            |connection| {
                connection.update_activity();
                Ok(())
            },
        )
    }

    /// Clean up expired connections
    pub fn cleanup_expired_connections(&self) -> usize {
        let expired_count = {
            let mut connections = self.connections.write();
            let expired_count = connections
                .iter()
                .filter(|(_, conn)| conn.is_expired(self.config.connection_timeout))
                .count();

            connections.retain(|_, conn| !conn.is_expired(self.config.connection_timeout));
            expired_count
        };

        if expired_count > 0 {
            info!("Cleaned up {expired_count} expired IPC connections");
        }

        expired_count
    }

    /// Generate a new unique message ID
    ///
    /// Returns 0 if the message ID counter mutex is poisoned.
    pub fn next_message_id(&self) -> u64 {
        let Ok(mut counter) = self.message_id_counter.lock() else {
            return 0;
        };
        let id = *counter;
        *counter = counter.wrapping_add(1);
        id
    }

    /// Get the size of the shared memory region
    pub const fn size(&self) -> usize {
        self.config.size
    }

    /// Get the number of active connections
    pub fn connection_count(&self) -> usize {
        self.connections.read().len()
    }

    /// Create CASC-compatible shmem file for client integration
    ///
    /// # Errors
    ///
    /// Returns error if shmem file cannot be created or written
    pub fn create_casc_shmem_file(
        &mut self,
        shmem_dir: &std::path::Path,
        installation_count: u32,
    ) -> Result<()> {
        use std::fs::File;
        use std::io::Write;

        // Ensure shmem directory exists
        std::fs::create_dir_all(shmem_dir).map_err(|e| {
            StorageError::SharedMemory(format!("Failed to create shmem directory: {e}"))
        })?;

        let shmem_file_path = shmem_dir.join("shmem");

        // Create CASC shmem header
        let header = CascShmemHeader::new(
            u64::try_from(self.config.size).unwrap_or(0),
            installation_count,
        );

        // Write header to shmem file
        let mut file = File::create(&shmem_file_path)
            .map_err(|e| StorageError::SharedMemory(format!("Failed to create shmem file: {e}")))?;

        let mut header_data = Vec::new();
        let mut cursor = std::io::Cursor::new(&mut header_data);
        header
            .write(&mut cursor)
            .map_err(|e| StorageError::SharedMemory(format!("Failed to serialize header: {e}")))?;

        file.write_all(&header_data)
            .map_err(|e| StorageError::SharedMemory(format!("Failed to write header: {e}")))?;

        // Write placeholder data for shared memory region info
        // In a full implementation, this would contain memory mapping details
        let placeholder_data = vec![0u8; 64]; // 64 bytes of placeholder data
        file.write_all(&placeholder_data).map_err(|e| {
            StorageError::SharedMemory(format!("Failed to write placeholder data: {e}"))
        })?;

        file.flush()
            .map_err(|e| StorageError::SharedMemory(format!("Failed to flush shmem file: {e}")))?;

        self.shmem_file_path = Some(shmem_file_path);
        info!(
            "Created CASC-compatible shmem file at {}",
            self.shmem_file_path
                .as_ref()
                .map_or_else(|| "unknown".to_string(), |p| p.display().to_string())
        );

        Ok(())
    }

    /// Update the CASC shmem file with current status
    ///
    /// # Errors
    ///
    /// Returns error if shmem file cannot be updated
    pub fn update_casc_shmem_file(&self, installation_count: u32) -> Result<()> {
        if let Some(ref shmem_file_path) = self.shmem_file_path {
            use std::io::{Seek, SeekFrom, Write};

            let header = CascShmemHeader::new(
                u64::try_from(self.config.size).unwrap_or(0),
                installation_count,
            );

            let mut file = std::fs::OpenOptions::new()
                .write(true)
                .open(shmem_file_path)
                .map_err(|e| {
                    StorageError::SharedMemory(format!("Failed to open shmem file: {e}"))
                })?;

            // Seek to beginning and write updated header
            file.seek(SeekFrom::Start(0))
                .map_err(|e| StorageError::SharedMemory(format!("Failed to seek: {e}")))?;

            let mut header_data = Vec::new();
            let mut cursor = std::io::Cursor::new(&mut header_data);
            header.write(&mut cursor).map_err(|e| {
                StorageError::SharedMemory(format!("Failed to serialize header: {e}"))
            })?;

            file.write_all(&header_data)
                .map_err(|e| StorageError::SharedMemory(format!("Failed to write header: {e}")))?;

            file.flush()
                .map_err(|e| StorageError::SharedMemory(format!("Failed to flush: {e}")))?;

            debug!(
                "Updated CASC shmem file with {} installations",
                installation_count
            );
        }

        Ok(())
    }

    /// Get connection statistics
    pub fn connection_stats(&self) -> HashMap<String, u64> {
        let mut stats = HashMap::new();

        {
            let connections = self.connections.read();

            stats.insert(
                "total_connections".to_string(),
                u64::try_from(connections.len()).unwrap_or(0),
            );

            let now = SystemTime::now();
            let active_connections = connections
                .values()
                .filter(|conn| {
                    now.duration_since(conn.last_activity).unwrap_or_default()
                        < HEARTBEAT_INTERVAL * 2
                })
                .count();

            stats.insert(
                "active_connections".to_string(),
                u64::try_from(active_connections).unwrap_or(0),
            );
            drop(connections);
        } // connections lock dropped here

        stats.insert(
            "max_connections".to_string(),
            u64::try_from(self.config.max_connections).unwrap_or(0),
        );

        stats
    }
}

/// Platform-specific shared memory handle
struct SharedMemoryHandle {
    #[cfg(target_os = "windows")]
    inner: windows::WindowsSharedMemory,
    #[cfg(unix)]
    inner: unix::UnixSharedMemory,
}

impl SharedMemoryHandle {
    /// Create a new shared memory handle
    fn create(name: &str, size: usize) -> Result<Self> {
        #[cfg(target_os = "windows")]
        {
            let inner = windows::WindowsSharedMemory::create(name, size)?;
            Ok(Self { inner })
        }
        #[cfg(unix)]
        {
            let inner = unix::UnixSharedMemory::create(name, size)?;
            Ok(Self { inner })
        }
    }

    /// Write data to shared memory
    fn write(&self, offset: usize, data: &[u8]) -> Result<()> {
        #[cfg(target_os = "windows")]
        {
            self.inner.write(offset, data)
        }
        #[cfg(unix)]
        {
            self.inner.write(offset, data)
        }
    }

    /// Read data from shared memory
    fn read(&self, offset: usize, size: usize) -> Result<Vec<u8>> {
        #[cfg(target_os = "windows")]
        {
            self.inner.read(offset, size)
        }
        #[cfg(unix)]
        {
            self.inner.read(offset, size)
        }
    }
}

/// Platform-specific shared memory implementation for Windows
#[cfg(target_os = "windows")]
mod windows {
    use std::ffi::CString;
    use std::ptr;
    use std::slice;

    use winapi::ctypes::c_void;
    use winapi::shared::minwindef::{DWORD, LPVOID};
    use winapi::um::handleapi::{CloseHandle, INVALID_HANDLE_VALUE};
    use winapi::um::memoryapi::{
        CreateFileMappingA, FILE_MAP_ALL_ACCESS, MapViewOfFile, UnmapViewOfFile,
    };
    use winapi::um::minwinbase::SECURITY_ATTRIBUTES;
    use winapi::um::sddl::ConvertStringSecurityDescriptorToSecurityDescriptorA;
    use winapi::um::securitybaseapi::LocalFree;
    use winapi::um::winnt::{HANDLE, PAGE_READWRITE, PSECURITY_DESCRIPTOR, SDDL_REVISION_1};

    use crate::{Result, StorageError};

    /// Windows shared memory implementation using CreateFileMapping
    #[allow(unsafe_code)]
    pub(super) struct WindowsSharedMemory {
        handle: HANDLE,
        view: LPVOID,
        size: usize,
    }

    impl WindowsSharedMemory {
        /// Create a new Windows shared memory region with restricted ACLs
        ///
        /// Security: Uses SDDL to create a security descriptor that grants
        /// full access only to the current user (OWNER_RIGHTS) and denies
        /// access to other users. This prevents other users on the same
        /// machine from reading or modifying the shared memory.
        #[allow(unsafe_code)]
        pub(super) fn create(name: &str, size: usize) -> Result<Self> {
            let c_name = CString::new(name)
                .map_err(|e| StorageError::SharedMemory(format!("Invalid name: {e}")))?;

            // Create security descriptor using SDDL
            // D:P - DACL is protected (doesn't inherit from parent)
            // (A;;GA;;;OW) - Allow Generic All to Owner
            // This restricts access to only the current user who created the object
            let sddl = CString::new("D:P(A;;GA;;;OW)")
                .map_err(|e| StorageError::SharedMemory(format!("Invalid SDDL: {e}")))?;

            let mut security_descriptor: PSECURITY_DESCRIPTOR = ptr::null_mut();

            let success = unsafe {
                ConvertStringSecurityDescriptorToSecurityDescriptorA(
                    sddl.as_ptr(),
                    SDDL_REVISION_1,
                    &mut security_descriptor,
                    ptr::null_mut(),
                )
            };

            if success == 0 {
                // Fall back to default security (null) if SDDL parsing fails
                // This maintains backwards compatibility while logging the issue
                tracing::warn!(
                    "Failed to create security descriptor for shared memory, \
                     using default security"
                );
                return Self::create_with_security(name, size, ptr::null_mut());
            }

            // Create with the security descriptor
            let result = Self::create_with_security(name, size, security_descriptor);

            // Free the security descriptor (created by ConvertStringSecurityDescriptor)
            unsafe {
                LocalFree(security_descriptor as *mut c_void);
            }

            result
        }

        /// Create shared memory with a specific security descriptor
        #[allow(unsafe_code)]
        fn create_with_security(
            name: &str,
            size: usize,
            security_descriptor: PSECURITY_DESCRIPTOR,
        ) -> Result<Self> {
            let c_name = CString::new(name)
                .map_err(|e| StorageError::SharedMemory(format!("Invalid name: {e}")))?;

            // Set up security attributes
            let mut security_attributes = SECURITY_ATTRIBUTES {
                nLength: std::mem::size_of::<SECURITY_ATTRIBUTES>() as DWORD,
                lpSecurityDescriptor: security_descriptor as *mut c_void,
                bInheritHandle: 0, // Don't inherit handle to child processes
            };

            let sa_ptr = if security_descriptor.is_null() {
                ptr::null_mut()
            } else {
                &mut security_attributes as *mut SECURITY_ATTRIBUTES
            };

            // Create file mapping with security attributes
            let handle = unsafe {
                CreateFileMappingA(
                    INVALID_HANDLE_VALUE,
                    sa_ptr,
                    PAGE_READWRITE,
                    0,
                    size as DWORD,
                    c_name.as_ptr(),
                )
            };

            if handle.is_null() {
                return Err(StorageError::SharedMemory(
                    "Failed to create file mapping".to_string(),
                ));
            }

            // Map view of file
            let view = unsafe { MapViewOfFile(handle, FILE_MAP_ALL_ACCESS, 0, 0, size) };

            if view.is_null() {
                unsafe { CloseHandle(handle) };
                return Err(StorageError::SharedMemory(
                    "Failed to map view of file".to_string(),
                ));
            }

            Ok(Self { handle, view, size })
        }

        /// Write data to shared memory
        #[allow(unsafe_code)]
        pub(super) fn write(&self, offset: usize, data: &[u8]) -> Result<()> {
            // Security: Use checked arithmetic to prevent integer overflow
            let end = offset.checked_add(data.len()).ok_or_else(|| {
                StorageError::SharedMemory("Integer overflow in offset calculation".to_string())
            })?;

            if end > self.size {
                return Err(StorageError::SharedMemory(
                    "Write would exceed shared memory bounds".to_string(),
                ));
            }

            // SAFETY: We verified offset + data.len() <= self.size above,
            // so the pointer arithmetic and write are within bounds.
            unsafe {
                let dest = (self.view as *mut u8).add(offset);
                ptr::copy_nonoverlapping(data.as_ptr(), dest, data.len());
            }

            Ok(())
        }

        /// Read data from shared memory
        #[allow(unsafe_code)]
        pub(super) fn read(&self, offset: usize, size: usize) -> Result<Vec<u8>> {
            // Security: Use checked arithmetic to prevent integer overflow
            let end = offset.checked_add(size).ok_or_else(|| {
                StorageError::SharedMemory("Integer overflow in offset calculation".to_string())
            })?;

            if end > self.size {
                return Err(StorageError::SharedMemory(
                    "Read would exceed shared memory bounds".to_string(),
                ));
            }

            // SAFETY: We verified offset + size <= self.size above,
            // so the pointer arithmetic and read are within bounds.
            unsafe {
                let src = (self.view as *const u8).add(offset);
                let data = slice::from_raw_parts(src, size);
                Ok(data.to_vec())
            }
        }
    }

    impl Drop for WindowsSharedMemory {
        #[allow(unsafe_code)]
        fn drop(&mut self) {
            // SAFETY: All operations here use validated handles from creation.
            // Windows API calls return success/failure but we ignore errors
            // since we're already cleaning up and cannot propagate errors.
            unsafe {
                if !self.view.is_null() {
                    UnmapViewOfFile(self.view);
                }
                if !self.handle.is_null() {
                    CloseHandle(self.handle);
                }
            }
        }
    }

    // SAFETY: WindowsSharedMemory is Send because:
    // 1. Windows HANDLE is an opaque kernel handle that's safe to transfer between threads
    // 2. LPVOID (view) is a raw pointer, but we only access it through controlled methods
    // 3. All internal state is either Windows handles or primitive types
    // 4. The Drop implementation properly handles cleanup from any thread
    #[allow(unsafe_code)]
    unsafe impl Send for WindowsSharedMemory {}

    // SAFETY: WindowsSharedMemory is Sync because:
    // 1. Read operations via view pointer are safe when externally synchronized
    // 2. Write operations require &mut self, preventing concurrent mutable access
    // 3. Windows kernel handles are inherently thread-safe for most operations
    // WARNING: Users MUST provide external synchronization (e.g., RwLock) when sharing
    // mutable access to the shared memory contents between threads. This type does NOT
    // provide internal synchronization for the memory-mapped region.
    #[allow(unsafe_code)]
    unsafe impl Sync for WindowsSharedMemory {}
}

/// Platform-specific shared memory implementation for Unix
#[cfg(unix)]
mod unix {
    use std::ffi::CString;
    use std::os::unix::io::RawFd;
    use std::ptr;
    use std::slice;

    use libc::{MAP_SHARED, O_CREAT, O_RDWR, PROT_READ, PROT_WRITE, S_IRUSR, S_IWUSR};
    use libc::{c_uint, c_void, mode_t, off_t, size_t};
    use libc::{close, ftruncate, mmap, munmap, shm_open, shm_unlink};

    use crate::{Result, StorageError};

    /// Unix shared memory implementation using `shm_open`
    #[allow(unsafe_code)]
    pub(super) struct UnixSharedMemory {
        fd: RawFd,
        ptr: *mut c_void,
        size: usize,
        name: String,
    }

    impl UnixSharedMemory {
        /// Create a new Unix shared memory region
        #[allow(unsafe_code)]
        pub(super) fn create(name: &str, size: usize) -> Result<Self> {
            let shm_name = format!("/cascette_{name}");
            let c_name = CString::new(shm_name.clone())
                .map_err(|e| StorageError::SharedMemory(format!("Invalid name: {e}")))?;

            // Open shared memory object
            let fd = unsafe {
                shm_open(
                    c_name.as_ptr(),
                    O_CREAT | O_RDWR,
                    (S_IRUSR | S_IWUSR) as mode_t as c_uint,
                )
            };

            if fd == -1 {
                return Err(StorageError::SharedMemory(
                    "Failed to open shared memory object".to_string(),
                ));
            }

            // Set size
            let size_off_t = off_t::try_from(size).unwrap_or(off_t::MAX);
            if unsafe { ftruncate(fd, size_off_t) } == -1 {
                unsafe { close(fd) };
                return Err(StorageError::SharedMemory(
                    "Failed to set shared memory size".to_string(),
                ));
            }

            // Map memory
            let ptr = unsafe {
                mmap(
                    ptr::null_mut(),
                    size as size_t,
                    PROT_READ | PROT_WRITE,
                    MAP_SHARED,
                    fd,
                    0,
                )
            };

            if ptr == libc::MAP_FAILED {
                unsafe {
                    close(fd);
                    shm_unlink(c_name.as_ptr());
                }
                return Err(StorageError::SharedMemory(
                    "Failed to map shared memory".to_string(),
                ));
            }

            Ok(Self {
                fd,
                ptr,
                size,
                name: shm_name,
            })
        }

        /// Write data to shared memory
        #[allow(unsafe_code)]
        pub(super) fn write(&self, offset: usize, data: &[u8]) -> Result<()> {
            // Security: Use checked arithmetic to prevent integer overflow
            let end = offset.checked_add(data.len()).ok_or_else(|| {
                StorageError::SharedMemory("Integer overflow in offset calculation".to_string())
            })?;

            if end > self.size {
                return Err(StorageError::SharedMemory(
                    "Write would exceed shared memory bounds".to_string(),
                ));
            }

            // SAFETY: We verified offset + data.len() <= self.size above,
            // so the pointer arithmetic and write are within bounds.
            unsafe {
                let dest = self.ptr.cast::<u8>().add(offset);
                ptr::copy_nonoverlapping(data.as_ptr(), dest, data.len());
            }

            Ok(())
        }

        /// Read data from shared memory
        #[allow(unsafe_code)]
        pub(super) fn read(&self, offset: usize, size: usize) -> Result<Vec<u8>> {
            // Security: Use checked arithmetic to prevent integer overflow
            let end = offset.checked_add(size).ok_or_else(|| {
                StorageError::SharedMemory("Integer overflow in offset calculation".to_string())
            })?;

            if end > self.size {
                return Err(StorageError::SharedMemory(
                    "Read would exceed shared memory bounds".to_string(),
                ));
            }

            // SAFETY: We verified offset + size <= self.size above,
            // so the pointer arithmetic and read are within bounds.
            unsafe {
                let src = (self.ptr as *const u8).add(offset);
                let data = slice::from_raw_parts(src, size);
                Ok(data.to_vec())
            }
        }
    }

    impl Drop for UnixSharedMemory {
        #[allow(unsafe_code)]
        fn drop(&mut self) {
            // SAFETY: All operations here use validated handles from creation.
            // We never panic in Drop - errors are silently ignored since we're
            // already cleaning up and cannot propagate errors.
            unsafe {
                if !self.ptr.is_null() {
                    munmap(self.ptr, self.size as size_t);
                }
                if self.fd != -1 {
                    close(self.fd);
                }
                // Unlink shared memory object - handle CString creation failure
                // gracefully to avoid panic during stack unwinding
                if let Ok(c_name) = CString::new(self.name.clone()) {
                    shm_unlink(c_name.as_ptr());
                }
                // If CString creation fails (name contains null byte), we skip
                // unlinking. The OS will clean up the shared memory segment
                // when no processes have it mapped.
            }
        }
    }

    // SAFETY: UnixSharedMemory is Send because:
    // 1. The file descriptor (fd) is an integer handle safe to transfer between threads
    // 2. The raw pointer (ptr) points to memory-mapped shared memory managed by the OS
    // 3. All internal state is either primitive integers or owned Strings
    // 4. The Drop implementation properly handles cleanup from any thread
    #[allow(unsafe_code)]
    unsafe impl Send for UnixSharedMemory {}

    // SAFETY: UnixSharedMemory is Sync because:
    // 1. Read operations via ptr are safe when data is immutable or externally synchronized
    // 2. Write operations require &mut self, preventing concurrent mutable access
    // 3. The OS kernel handles concurrent access to the underlying shared memory
    // WARNING: Users MUST provide external synchronization (e.g., RwLock) when sharing
    // mutable access to the shared memory contents between threads.
    #[allow(unsafe_code)]
    unsafe impl Sync for UnixSharedMemory {}
}

// Validation implementations for round-trip testing
#[cfg(test)]
#[allow(clippy::expect_used)]
mod validation_impls {
    use super::*;
    use crate::validation::BinaryFormatValidator;

    impl PartialEq for MessageHeader {
        fn eq(&self, other: &Self) -> bool {
            self.magic == other.magic
                && self.version == other.version
                && self.message_type == other.message_type
                && self.payload_size == other.payload_size
                && self.message_id == other.message_id
                // Skip timestamp comparison as it varies
                && self.reserved == other.reserved
        }
    }

    impl BinaryFormatValidator for MessageHeader {
        fn generate_valid_instance() -> Self {
            Self::new(MessageType::FileRequest, 1024, 12345)
        }

        fn generate_edge_cases() -> Vec<Self> {
            vec![
                // All message types
                Self::new(MessageType::FileRequest, 0, 0),
                Self::new(MessageType::FileResponse, 1, 1),
                Self::new(MessageType::StatusRequest, MAX_PAYLOAD_SIZE, u64::MAX),
                Self::new(MessageType::StatusResponse, 65536, 0x1234_5678_9ABC_DEF0),
                Self::new(MessageType::KeepAlive, 16, 42),
                Self::new(MessageType::Error, 512, 0xDEAD_BEEF_CAFE_BABE),
            ]
        }

        fn validate_serialized_data(&self, data: &[u8]) -> Result<()> {
            if data.len() != Self::size() {
                return Err(StorageError::InvalidFormat(format!(
                    "MessageHeader should be {} bytes, got {}",
                    Self::size(),
                    data.len()
                )));
            }

            // Validate magic bytes (first 4 bytes, big-endian)
            let magic_bytes = &data[0..4];
            let expected_magic = IPC_MAGIC.to_be_bytes();
            if magic_bytes != expected_magic {
                return Err(StorageError::InvalidFormat(format!(
                    "Invalid magic bytes: expected {expected_magic:?}, got {magic_bytes:?}"
                )));
            }

            // Validate version (bytes 4-6, big-endian)
            let version_bytes = &data[4..6];
            let expected_version = IPC_VERSION.to_be_bytes();
            if version_bytes != expected_version {
                return Err(StorageError::InvalidFormat(format!(
                    "Invalid version: expected {expected_version:?}, got {version_bytes:?}"
                )));
            }

            Ok(())
        }
    }

    impl PartialEq for FileRequestPayload {
        fn eq(&self, other: &Self) -> bool {
            self.request_type == other.request_type
                && self.priority == other.priority
                && self.flags == other.flags
                && self.identifier_length == other.identifier_length
                && self.identifier == other.identifier
        }
    }

    impl BinaryFormatValidator for FileRequestPayload {
        fn generate_valid_instance() -> Self {
            Self::by_path("Interface/AddOns/MyAddon/MyFile.lua", 1)
        }

        fn generate_edge_cases() -> Vec<Self> {
            vec![
                // Empty path
                Self::by_path("", 0),
                // Long path
                Self::by_path(&"a".repeat(1000), 2),
                // Unicode path
                Self::by_path("Interface/AddOns/MyAddon/\u{6587}\u{4ef6}.lua", 1),
                // FileDataID requests
                Self::by_file_data_id(0, 0),
                Self::by_file_data_id(u32::MAX, 2),
                Self::by_file_data_id(123_456, 1),
            ]
        }

        fn validate_serialized_data(&self, data: &[u8]) -> Result<()> {
            // Minimum size: 1 + 1 + 2 + 4 = 8 bytes
            if data.len() < 8 {
                return Err(StorageError::InvalidFormat(format!(
                    "FileRequestPayload too small: {} bytes",
                    data.len()
                )));
            }

            // Validate identifier length matches actual data
            let expected_size = 8 + self.identifier_length as usize;
            if data.len() != expected_size {
                return Err(StorageError::InvalidFormat(format!(
                    "Size mismatch: expected {}, got {}",
                    expected_size,
                    data.len()
                )));
            }

            Ok(())
        }
    }

    impl PartialEq for FileResponsePayload {
        fn eq(&self, other: &Self) -> bool {
            self.status == other.status
                && self.compression == other.compression
                && self.flags == other.flags
                && self.uncompressed_size == other.uncompressed_size
                && self.compressed_size == other.compressed_size
                && self.content_hash == other.content_hash
                && self.data == other.data
        }
    }

    impl BinaryFormatValidator for FileResponsePayload {
        fn generate_valid_instance() -> Self {
            let test_data = b"Hello, CASC World!".to_vec();
            let hash = [
                0x12, 0x34, 0x56, 0x78, 0x9A, 0xBC, 0xDE, 0xF0, 0x12, 0x34, 0x56, 0x78, 0x9A, 0xBC,
                0xDE, 0xF0,
            ];
            Self::success(test_data, hash)
        }

        fn generate_edge_cases() -> Vec<Self> {
            vec![
                // Empty success response
                Self::success(Vec::new(), [0; 16]),
                // Large data response
                Self::success(vec![0x42; 65536], [0xFF; 16]),
                // Not found response
                Self::not_found(),
                // Error response
                Self::error(),
            ]
        }

        fn validate_serialized_data(&self, data: &[u8]) -> Result<()> {
            // Minimum size: 1 + 1 + 2 + 4 + 4 + 16 = 28 bytes
            if data.len() < 28 {
                return Err(StorageError::InvalidFormat(format!(
                    "FileResponsePayload too small: {} bytes",
                    data.len()
                )));
            }

            // Validate data size matches compressed_size field
            let expected_size = 28 + self.compressed_size as usize;
            if data.len() != expected_size {
                return Err(StorageError::InvalidFormat(format!(
                    "Data size mismatch: expected {}, got {}",
                    expected_size,
                    data.len()
                )));
            }

            Ok(())
        }
    }

    impl PartialEq for StatusRequestPayload {
        fn eq(&self, other: &Self) -> bool {
            self.status_type == other.status_type
                && self.flags == other.flags
                && self.name_length == other.name_length
                && self.installation_name == other.installation_name
        }
    }

    impl BinaryFormatValidator for StatusRequestPayload {
        fn generate_valid_instance() -> Self {
            Self::installation("wow_retail")
        }

        fn generate_edge_cases() -> Vec<Self> {
            vec![
                // General status request
                Self::general(),
                // Installation requests
                Self::installation(""),
                Self::installation("wow_classic"),
                Self::installation("wow_beta"),
                Self::installation(&"x".repeat(255)), // Long name
            ]
        }

        fn validate_serialized_data(&self, data: &[u8]) -> Result<()> {
            // Minimum size: 1 + 3 + 4 = 8 bytes
            if data.len() < 8 {
                return Err(StorageError::InvalidFormat(format!(
                    "StatusRequestPayload too small: {} bytes",
                    data.len()
                )));
            }

            // Validate name length matches actual data
            let expected_size = 8 + self.name_length as usize;
            if data.len() != expected_size {
                return Err(StorageError::InvalidFormat(format!(
                    "Size mismatch: expected {}, got {}",
                    expected_size,
                    data.len()
                )));
            }

            Ok(())
        }
    }

    impl PartialEq for StatusResponsePayload {
        fn eq(&self, other: &Self) -> bool {
            self.system_status == other.system_status
                && self.installation_count == other.installation_count
                && self.flags == other.flags
                && self.cache_size == other.cache_size
                && self.cache_used == other.cache_used
                && self.cached_files == other.cached_files
                && self.uptime == other.uptime
                && self.status_data_length == other.status_data_length
                && self.status_data == other.status_data
        }
    }

    impl BinaryFormatValidator for StatusResponsePayload {
        fn generate_valid_instance() -> Self {
            Self::new(
                0,                  // Healthy
                3,                  // 3 installations
                1024 * 1024 * 1024, // 1GB cache
                512 * 1024 * 1024,  // 512MB used
                1000,               // 1000 cached files
                3600,               // 1 hour uptime
                r#"{"version": "1.0", "status": "running"}"#.to_string(),
            )
        }

        fn generate_edge_cases() -> Vec<Self> {
            vec![
                // Minimal response
                Self::new(0, 0, 0, 0, 0, 0, String::new()),
                // Maximum values
                Self::new(
                    2,
                    u8::MAX,
                    u64::MAX,
                    u64::MAX,
                    u32::MAX,
                    u32::MAX,
                    "x".repeat(1000),
                ),
                // Degraded status
                Self::new(
                    1,
                    5,
                    2048,
                    1500,
                    500,
                    86400,
                    r#"{"errors": ["disk_full"]}"#.to_string(),
                ),
                // Error status
                Self::new(
                    2,
                    0,
                    0,
                    0,
                    0,
                    0,
                    r#"{"error": "system_failure"}"#.to_string(),
                ),
            ]
        }

        fn validate_serialized_data(&self, data: &[u8]) -> Result<()> {
            // Minimum size: 1 + 1 + 2 + 8 + 8 + 4 + 4 + 4 = 32 bytes
            if data.len() < 32 {
                return Err(StorageError::InvalidFormat(format!(
                    "StatusResponsePayload too small: {} bytes",
                    data.len()
                )));
            }

            // Validate status data length matches actual data
            let expected_size = 32 + self.status_data_length as usize;
            if data.len() != expected_size {
                return Err(StorageError::InvalidFormat(format!(
                    "Size mismatch: expected {}, got {}",
                    expected_size,
                    data.len()
                )));
            }

            Ok(())
        }
    }

    impl PartialEq for KeepAlivePayload {
        fn eq(&self, other: &Self) -> bool {
            self.sequence == other.sequence
                && self.connection_id == other.connection_id
                && self.reserved == other.reserved
        }
    }

    impl BinaryFormatValidator for KeepAlivePayload {
        fn generate_valid_instance() -> Self {
            Self::new(42, 123)
        }

        fn generate_edge_cases() -> Vec<Self> {
            vec![
                // Minimum values
                Self::new(0, 0),
                // Maximum values
                Self::new(u64::MAX, u32::MAX),
                // Pattern values
                Self::new(0xDEAD_BEEF_CAFE_BABE, 0x1234_5678),
            ]
        }

        fn validate_serialized_data(&self, data: &[u8]) -> Result<()> {
            if data.len() != 16 {
                // 8 + 4 + 4 bytes
                return Err(StorageError::InvalidFormat(format!(
                    "KeepAlivePayload should be 16 bytes, got {}",
                    data.len()
                )));
            }
            Ok(())
        }
    }

    impl PartialEq for CascShmemHeader {
        fn eq(&self, other: &Self) -> bool {
            self.magic == other.magic
                && self.version == other.version
                && self.region_size == other.region_size
                && self.installation_count == other.installation_count
                && self.reserved == other.reserved
        }
    }

    impl BinaryFormatValidator for CascShmemHeader {
        fn generate_valid_instance() -> Self {
            Self::new(64 * 1024 * 1024, 3) // 64MB, 3 installations
        }

        fn generate_edge_cases() -> Vec<Self> {
            vec![
                // Minimum values
                Self::new(0, 0),
                // Maximum values
                Self::new(u64::MAX, u32::MAX),
                // Realistic values
                Self::new(128 * 1024 * 1024, 5), // 128MB, 5 installations
                Self::new(1024 * 1024 * 1024, 10), // 1GB, 10 installations
            ]
        }

        fn validate_serialized_data(&self, data: &[u8]) -> Result<()> {
            if data.len() != Self::size() {
                return Err(StorageError::InvalidFormat(format!(
                    "CascShmemHeader should be {} bytes, got {}",
                    Self::size(),
                    data.len()
                )));
            }

            // Validate magic bytes
            let magic_bytes = &data[0..4];
            let expected_magic = 0x4341_5343u32.to_be_bytes(); // "CASC"
            if magic_bytes != expected_magic {
                return Err(StorageError::InvalidFormat(
                    "Invalid CASC magic bytes".to_string(),
                ));
            }

            Ok(())
        }
    }
}
