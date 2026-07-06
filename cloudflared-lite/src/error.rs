use thiserror::Error;

#[derive(Error, Debug)]
pub enum TunnelError {
    #[error("API error: {0}")]
    Api(#[from] reqwest::Error),

    #[error("API business error: {0:?}")]
    ApiBusiness(Vec<String>),

    #[error("API non-JSON response: status={status} body={body}")]
    ApiNonJson { status: u16, body: String },

    #[error("edge discovery failed: {0}")]
    Discovery(String),

    #[error("QUIC dial failed after {attempts} attempt(s): {last}")]
    QuicDial { attempts: u32, last: String },

    #[error("registration failed: {0}")]
    Register(String),

    #[error("permanent failure after {0} attempt(s)")]
    PermanentFailure(u32),

    #[error("tunnel shutdown")]
    Shutdown,

    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),

    #[error("internal error: {0}")]
    Internal(String),

    #[error("token decode error: {0}")]
    TokenDecode(String),

    #[error("config error: {0}")]
    Config(String),
}

pub type Result<T> = std::result::Result<T, TunnelError>;
