//! Intermediate results of the check steps (`TcpStep`, `TlsStep`, `BodyMark`) and their assembly into `Steps` —
//! the input of the pure classification in `classify.rs`.

/// Result of the TCP step.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TcpStep {
    NotTried,
    Ok,
    Refused,
    Reset,
    Timeout,
}

/// Result of the TLS step.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TlsStep {
    NotTried,
    Ok,
    /// The server, or whoever posed as the server, sent an alert.
    Alert(String),
    /// The connection closed in the middle of the handshake.
    Closed,
    Timeout,
    /// The certificate failed verification or the protocol was violated.
    Invalid(String),
}

/// What was found in the response body.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BodyMark {
    Clean,
    Antibot,
    Geo,
}

/// The collected steps — the input of classification.
#[derive(Debug, Clone)]
pub struct Steps {
    pub dns_ok: bool,
    pub dns_mismatch: bool,
    pub unusable_addr: bool,
    pub tcp: TcpStep,
    pub tls: TlsStep,
    pub http_status: u16,
    pub bytes_received: u32,
    pub stalled: bool,
    pub body_mark: BodyMark,
}

impl Default for Steps {
    fn default() -> Self {
        Steps {
            dns_ok: false,
            dns_mismatch: false,
            unusable_addr: false,
            tcp: TcpStep::NotTried,
            tls: TlsStep::NotTried,
            http_status: 0,
            bytes_received: 0,
            stalled: false,
            body_mark: BodyMark::Clean,
        }
    }
}
