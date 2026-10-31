//! The app's one error (web/app.rb's handlers): what went wrong and the
//! status it renders the Erratum with.

#[derive(Debug, Clone)]
pub enum AppError {
    /// `halt_with(status, message)`.
    Halt(u16, String),
    /// A `Cistergraphia::Error`: the reader's mistake, 400.
    Domain(String),
    /// A route miss.
    NotFound,
    /// A real bug: 500, never the exception's own words.
    Bug,
}

impl AppError {
    pub fn halt(status: u16, message: impl Into<String>) -> AppError {
        AppError::Halt(status, message.into())
    }

    pub fn status(&self) -> u16 {
        match self {
            AppError::Halt(status, _) => *status,
            AppError::Domain(_) => 400,
            AppError::NotFound => 404,
            AppError::Bug => 500,
        }
    }

    pub fn message(&self) -> String {
        match self {
            AppError::Halt(_, message) => message.clone(),
            AppError::Domain(message) => message.clone(),
            AppError::NotFound => "This page is not in the index.".to_string(),
            AppError::Bug => "Something unexpected happened, please try again.".to_string(),
        }
    }
}

impl From<rustergraphia::Error> for AppError {
    fn from(error: rustergraphia::Error) -> AppError {
        AppError::Domain(error.to_string())
    }
}
