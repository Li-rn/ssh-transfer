use thiserror::Error;

#[derive(Error, Debug)]
pub enum TransferError {
    #[error("Authentication failed")]
    AuthenticationFailed,
    
    #[error("Directory operations not allowed without recursive flag")]
    DirectoryNotAllowed,
    
    #[error("Thread join error")]
    ThreadJoinError,
    
    #[error("File not found: {path}")]
    FileNotFound { path: String },
    
    #[error("Permission denied: {path}")]
    PermissionDenied { path: String },
    
    #[error("Network error: {message}")]
    NetworkError { message: String },
    
    #[error("IO error: {0}")]
    IoError(#[from] std::io::Error),
    
    #[error("SSH error: {0}")]
    SshError(#[from] ssh2::Error),
}