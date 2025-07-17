use thiserror::Error;

#[derive(Error, Debug)]
pub enum Error {
    #[error("IO error: {0}")]
    IOError(#[from] std::io::Error),

    #[error("Not implemented")]
    NotImplemented,

    #[error("File ID delta over- or under-flows")]
    FileIdDeltaOverflow,

    #[error("File has incorrect magic - possibly wrong file format")]
    BadMagic,

    #[error("Failed precondition")]
    FailedPrecondition,

    #[error("Config parser syntax error")]
    ConfigSyntax,

    #[error("Config parser parameter type mismatch")]
    ConfigTypeMismatch,

    #[error("Block index {0} is out of range, must be less than {1}")]
    BlockIndexOutOfRange(u64, u64),

    #[error("Data checksum mismatch")]
    ChecksumMismatch,

    #[error("Unsupported BLTE encoding type: {0}")]
    UnsupportedBlteEncoding(u8),

    #[error("Listfile syntax error")]
    ListfileSyntax,

    #[error("Listfile file ID is not an integer")]
    InvalidListfileID,
}
