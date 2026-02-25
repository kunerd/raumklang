use std::fmt;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OutPort(String);

impl OutPort {
    pub fn new(name: String) -> Self {
        Self(name)
    }
}

impl fmt::Display for OutPort {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl AsRef<str> for OutPort {
    fn as_ref(&self) -> &str {
        self.0.as_ref()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InPort(String);

impl InPort {
    pub fn new(name: String) -> Self {
        Self(name)
    }
}

impl fmt::Display for InPort {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl AsRef<str> for InPort {
    fn as_ref(&self) -> &str {
        self.0.as_ref()
    }
}
