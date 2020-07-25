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

impl<T: AsRef<str>> From<T> for Error {
    fn from(error: T) -> Self {
        Error(vec![error.as_ref().to_string()])
    }
}

impl std::error::Error for Error {}
