use std::fmt;

// TODO Find a better type
#[derive(Debug)]
pub struct Error(pub Vec<String>);

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let message = self.0.join(" ");
        write!(f, "{}", message)
    }
}

impl From<&str> for Error {
    fn from(error: &str) -> Self {
        Error::from(error.to_string())
    }
}

impl From<String> for Error {
    fn from(error: String) -> Self {
        Error(vec![error])
    }
}

impl std::error::Error for Error {}
