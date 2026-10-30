//! The errors raised on bad user input, mirroring the Ruby exception tree:
//! every variant is an `ArgumentError` there, and the decode variants are
//! `DecodeError`s. `is_decode` tells the two apart where Ruby rescues
//! `DecodeError` alone.

/// Base error for every failure on bad user input.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum Error {
    /// Raised when a message contains nothing that can be enciphered.
    #[error("{0}")]
    EmptyMessage(String),
    /// Raised when an SVG or a sequence cannot be deciphered.
    #[error("{0}")]
    Decode(String),
    /// Raised when a secret plate does not open with the passphrase given.
    #[error("{0}")]
    WrongPassphrase(String),
    /// Raised when a plate written under a grille is read without its key.
    #[error("{0}")]
    GrilleKeyNeeded(String),
    /// Raised when a grille does not read a plate written under a grille.
    #[error("{0}")]
    WrongGrille(String),
    /// Any other bad input (Ruby's plain `Cistergraphia::Error`).
    #[error("{0}")]
    Other(String),
}

impl Error {
    pub fn decode(message: impl Into<String>) -> Self {
        Error::Decode(message.into())
    }

    pub fn other(message: impl Into<String>) -> Self {
        Error::Other(message.into())
    }

    /// Whether Ruby would rescue this as a `DecodeError`.
    pub fn is_decode(&self) -> bool {
        matches!(
            self,
            Error::Decode(_)
                | Error::WrongPassphrase(_)
                | Error::GrilleKeyNeeded(_)
                | Error::WrongGrille(_)
        )
    }
}

pub type Result<T> = std::result::Result<T, Error>;
