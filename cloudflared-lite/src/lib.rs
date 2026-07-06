#[allow(clippy::all, unused, non_camel_case_types, non_upper_case_globals, non_snake_case)]
pub mod tunnelrpc_capnp {
    include!("proto_gen/tunnelrpc_capnp.rs");
}

#[allow(clippy::all, unused, non_camel_case_types, non_upper_case_globals, non_snake_case)]
pub mod quic_metadata_protocol_capnp {
    include!("proto_gen/quic_metadata_protocol_capnp.rs");
}

pub mod config;
pub mod edge;
pub mod error;
pub mod metrics;
pub mod proxy;
pub mod rpc;
pub mod tunnel;

pub use error::TunnelError;
pub use tunnel::TunnelManager;
