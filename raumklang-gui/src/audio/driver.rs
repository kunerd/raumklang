#[derive(Debug, Clone)]
pub enum Notification {
    OutPortConnected(String),
    OutPortDisconnected,
    InPortConnected(String),
    InPortDisconnected,
}
