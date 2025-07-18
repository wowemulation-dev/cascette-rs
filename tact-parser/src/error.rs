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
}
