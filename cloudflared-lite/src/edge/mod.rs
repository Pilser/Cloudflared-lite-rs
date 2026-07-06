mod discover;
mod connect;

pub use discover::{discover_edges, EdgeAddr, EdgeRegistry};
pub use connect::{build_endpoint, dial_any, DialOptions};
