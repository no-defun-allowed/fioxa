use thiserror::Error;

#[derive(Debug, Clone, Error)]
pub enum LoadElfError {
    #[error("internal error")]
    InternalError,
}
