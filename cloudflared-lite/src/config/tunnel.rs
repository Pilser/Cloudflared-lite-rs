#[derive(Clone, Debug)]
pub struct TunnelConfig {
    pub account_tag: String,
    pub tunnel_secret: Vec<u8>,
    pub tunnel_id: Vec<u8>,
    pub hostname: String,
}
