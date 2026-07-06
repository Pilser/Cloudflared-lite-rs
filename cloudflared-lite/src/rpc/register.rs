use crate::config::TunnelConfig;
use crate::error::{Result, TunnelError};
use crate::tunnelrpc_capnp;
use capnp_rpc::{rpc_twoparty_capnp, twoparty, RpcSystem};
use std::time::Duration;
use tokio_util::compat::{TokioAsyncReadCompatExt, TokioAsyncWriteCompatExt};
use uuid::Uuid;

const RPC_TIMEOUT: Duration = Duration::from_secs(15);

struct StubCloudflaredServer;

impl tunnelrpc_capnp::session_manager::Server for StubCloudflaredServer {}
impl tunnelrpc_capnp::configuration_manager::Server for StubCloudflaredServer {}
impl tunnelrpc_capnp::cloudflared_server::Server for StubCloudflaredServer {}

#[derive(Debug, Clone)]
pub struct RegistrationDetails {
    pub uuid: Vec<u8>,
    pub location: String,
    pub tunnel_is_remotely_managed: bool,
}

pub struct Registration {
    pub details: RegistrationDetails,
    pub shutdown_tx: tokio::sync::oneshot::Sender<super::session::ShutdownCommand>,
    pub driver_done_rx: tokio::sync::oneshot::Receiver<()>,
    pub _join: std::thread::JoinHandle<()>,
}

pub async fn register_connection(
    conn: &quinn::Connection,
    config: &TunnelConfig,
    conn_index: u8,
    num_prev_attempts: u8,
) -> Result<Registration> {
    let (send, recv) = conn
        .open_bi()
        .await
        .map_err(|e| TunnelError::Register(format!("open_bi: {e}")))?;

    let tunnel_id = Uuid::from_slice(&config.tunnel_id)
        .map_err(|e| TunnelError::Register(format!("invalid tunnel_id uuid: {e}")))?;

    let (done_tx, done_rx) =
        tokio::sync::oneshot::channel::<std::result::Result<RegistrationDetails, TunnelError>>();
    let (shutdown_tx, shutdown_rx) =
        tokio::sync::oneshot::channel::<super::session::ShutdownCommand>();
    let (driver_done_tx, driver_done_rx) = tokio::sync::oneshot::channel::<()>();

    let auth_account_tag = config.account_tag.clone();
    let auth_secret = config.tunnel_secret.clone();

    let join = std::thread::Builder::new()
        .name("rpc-driver".into())
        .spawn(move || {
            let rt = tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .expect("rpc driver runtime");
            let local = tokio::task::LocalSet::new();
            local.block_on(&rt, async move {
                let reader = recv.compat();
                let writer = send.compat_write();
                let network = Box::new(twoparty::VatNetwork::new(
                    reader,
                    writer,
                    rpc_twoparty_capnp::Side::Client,
                    Default::default(),
                ));

                let stub: tunnelrpc_capnp::cloudflared_server::Client =
                    capnp_rpc::new_client(StubCloudflaredServer);

                let mut rpc_system = RpcSystem::new(network, Some(stub.client));
                let server: tunnelrpc_capnp::registration_server::Client =
                    rpc_system.bootstrap(rpc_twoparty_capnp::Side::Server);

                let mut request = server.register_connection_request();
                {
                    let mut params = request.get();
                    let mut auth = params.reborrow().init_auth();
                    auth.set_account_tag(&auth_account_tag);
                    auth.set_tunnel_secret(&auth_secret);

                    params.set_tunnel_id(tunnel_id.as_bytes());
                    params.set_conn_index(conn_index);

                    let mut opts = params.reborrow().init_options();
                    let mut client = opts.reborrow().init_client();
                    client.set_client_id(&Uuid::new_v4().as_bytes()[..]);
                    client.set_version(env!("CARGO_PKG_VERSION"));
                    client.set_arch(std::env::consts::ARCH);
                    opts.set_replace_existing(num_prev_attempts > 0);
                    opts.set_num_previous_attempts(num_prev_attempts);
                }

                let response_promise = request.send().promise;
                let call = async {
                    let reply = response_promise.await
                        .map_err(|e| TunnelError::Register(format!("RPC send: {e}")))?;
                    let response_reader = reply.get()
                        .map_err(|e| TunnelError::Register(format!("response root: {e}")))?;
                    let result = response_reader.get_result()
                        .map_err(|e| TunnelError::Register(format!("result: {e}")))?;
                    decode_connection_response(result)
                };

                tokio::pin!(call);
                tokio::pin!(shutdown_rx);
                let mut sent_done = false;
                let mut done_tx = Some(done_tx);
                let mut shutdown_kind: Option<super::session::ShutdownCommand> = None;

                loop {
                    tokio::select! {
                        biased;
                        res = &mut call, if !sent_done => {
                            if let Some(tx) = done_tx.take() {
                                let _ = tx.send(res);
                            }
                            sent_done = true;
                        }
                        cmd = &mut shutdown_rx => {
                            shutdown_kind = cmd.ok();
                            break;
                        }
                        _ = &mut rpc_system => {
                            if !sent_done {
                                if let Some(tx) = done_tx.take() {
                                    let _ = tx.send(Err(TunnelError::Register(
                                        "RPC system terminated before call completed".into(),
                                    )));
                                }
                            }
                            break;
                        }
                    }
                }

                if let Some(super::session::ShutdownCommand::Graceful(grace)) = shutdown_kind {
                    if sent_done {
                        let req = server.unregister_connection_request();
                        let _ = tokio::time::timeout(grace, req.send().promise).await;
                    }
                }

                drop(server);
                let _ = driver_done_tx.send(());
            });
        })
        .map_err(|e| TunnelError::Internal(format!("spawn rpc thread: {e}")))?;

    let details = tokio::time::timeout(RPC_TIMEOUT, done_rx)
        .await
        .map_err(|_| TunnelError::Register("RPC timed out".into()))?
        .map_err(|_| TunnelError::Register("RPC driver dropped channel".into()))??;

    Ok(Registration {
        details,
        shutdown_tx,
        driver_done_rx,
        _join: join,
    })
}

fn decode_connection_response(
    response: tunnelrpc_capnp::connection_response::Reader,
) -> std::result::Result<RegistrationDetails, TunnelError> {
    use tunnelrpc_capnp::connection_response::result::WhichReader;
    let result_reader = response.get_result();
    let tag = result_reader
        .which()
        .map_err(|e| TunnelError::Register(format!("result union: {e}")))?;
    match tag {
        WhichReader::Error(err_res) => {
            let err = err_res
                .map_err(|e| TunnelError::Register(format!("error reader: {e}")))?;
            let cause = err
                .get_cause()
                .ok()
                .and_then(|t| t.to_string().ok())
                .unwrap_or_else(|| "<missing cause>".into());
            Err(TunnelError::Register(cause))
        }
        WhichReader::ConnectionDetails(details_reader) => {
            let d = details_reader
                .map_err(|e| TunnelError::Register(format!("details reader: {e}")))?;
            let uuid = d
                .get_uuid()
                .map_err(|e| TunnelError::Register(format!("uuid: {e}")))?
                .to_vec();
            let location = d
                .get_location_name()
                .ok()
                .and_then(|t| t.to_string().ok())
                .unwrap_or_default();
            let managed = d.get_tunnel_is_remotely_managed();
            Ok(RegistrationDetails {
                uuid,
                location,
                tunnel_is_remotely_managed: managed,
            })
        }
    }
}
