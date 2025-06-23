#[derive(Debug, Clone)]
pub struct Config {
    out_port: String,
    in_port: String,
}

impl Config {
    pub fn new(out_port: Option<String>, in_port: Option<String>) -> Option<Config> {
        if let (Some(out_port), Some(in_port)) = (out_port, in_port) {
            Some(Config { out_port, in_port })
        } else {
            None
        }
    }
}
