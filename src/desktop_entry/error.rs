use std::fmt;

// TODO Find a better type
#[derive(Debug, Clone)]
pub struct Error(pub Vec<String>);

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let message = self.0.join(" ");
        let new_message = message.trim_end_matches('\n');
        write!(f, "{}", new_message)
    }
}

impl<T: AsRef<str>> From<T> for Error {
    fn from(error: T) -> Self {
        Error(vec![error.as_ref().to_string()])
    }
}

impl std::error::Error for Error {}

impl Error {
    pub fn push(&mut self, s: impl ToString) {
        self.0.push(s.to_string() + "\n");
    }

    pub fn len(&self) -> usize {
        self.0.len()
    }

    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }
}
