#![allow(unused_imports)]
use tracing::{info, warn, debug, error, trace, instrument, span, Level};
use tracing_futures::{Instrument, WithSubscriber};
use error_chain::bail;
use fxhash::FxHashMap;
#[cfg(feature = "enable_full")]
use tokio::{net::{TcpStream}};
use tokio::time::Duration;
use tokio::sync::Mutex;
use tokio::sync::broadcast;
use std::sync::Arc;
use serde::{Serialize, de::DeserializeOwned};
use std::net::SocketAddr;
#[cfg(not(feature="enable_dns"))]
use std::net::ToSocketAddrs;
use std::result::Result;
use parking_lot::Mutex as StdMutex;

use crate::{error::*, comms::NodeId};
use crate::crypto::*;
use crate::spec::*;
#[allow(unused_imports)]
use crate::conf::*;
use crate::engine::TaskEngine;

use super::{conf::*, hello::HelloMetadata};
use super::metrics::*;
use super::throttle::*;
use super::rx_tx::*;
use super::helper::*;
use super::hello;
use super::key_exchange;
use super::CertificateValidation;
#[allow(unused_imports)]
use {
    super::Stream,
    super::StreamRx,
    super::StreamTx,
    super::StreamTxChannel,
    super::StreamProtocol
};

pub(crate) async fn connect<M, C>
(
    conf: &MeshConfig,
    hello_path: String,
    node_id: NodeId,
    inbox: impl InboxProcessor<M, C> + 'static,
    metrics: Arc<StdMutex<Metrics>>,
    throttle: Arc<StdMutex<Throttle>>,
    exit: broadcast::Receiver<()>
)
-> Result<Tx, CommsError>
where M: Send + Sync + Serialize + DeserializeOwned + Default + Clone + 'static,
      C: Send + Sync + Default + 'static,
{
    // Create all the outbound connections
    if let Some(target) = &conf.connect_to
    {
        // Perform the connect operation
        let inbox = Box::new(inbox);
        let upstream = mesh_connect_to::<M, C>(
            target.clone(), 
            hello_path.clone(),
            node_id,
            conf.cfg_mesh.domain_name.clone(),
            inbox,
            conf.cfg_mesh.wire_protocol,
            conf.cfg_mesh.wire_encryption,
            conf.cfg_mesh.connect_timeout,
            conf.cfg_mesh.fail_fast,
            conf.cfg_mesh.certificate_validation.clone(),
            Arc::clone(&metrics),
            Arc::clone(&throttle),
            exit,
        ).await?;
        
        // Return the mesh
        Ok(
            Tx {
                direction: TxDirection::Upcast(upstream),
                hello_path: hello_path.clone(),
                wire_format: conf.cfg_mesh.wire_format,
                relay: None,
                metrics: Arc::clone(&metrics),
                throttle: Arc::clone(&throttle),
                exit_dependencies: Vec::new(),
            },
        )
    }
    else
    {
        bail!(CommsErrorKind::NoAddress);
    }
}

#[cfg(feature="enable_dns")]
type MeshConnectAddr = SocketAddr;
#[cfg(not(feature="enable_dns"))]
type MeshConnectAddr = crate::conf::MeshAddress;

pub(super) async fn mesh_connect_to<M, C>
(
    addr: MeshConnectAddr,
    hello_path: String,
    node_id: NodeId,
    domain: String,
    inbox: Box<dyn InboxProcessor<M, C>>,
    wire_protocol: StreamProtocol,
    wire_encryption: Option<KeySize>,
    timeout: Duration,
    fail_fast: bool,
    validation: CertificateValidation,
    metrics: Arc<StdMutex<super::metrics::Metrics>>,
    throttle: Arc<StdMutex<super::throttle::Throttle>>,
    exit: broadcast::Receiver<()>
)
-> Result<Upstream, CommsError>
where M: Send + Sync + Serialize + DeserializeOwned + Clone + Default + 'static,
      C: Send + Sync + Default + 'static,
{
    // Make the connection
    trace!("prepare connect (path={})", hello_path);
    let worker_connect = mesh_connect_prepare
    (
        addr.clone(),
        hello_path,
        node_id,
        domain,
        wire_protocol,
        wire_encryption,
        fail_fast,
    );
    let (mut worker_connect, mut stream_tx) = tokio::time::timeout(timeout, worker_connect).await??;
    let wire_format = worker_connect.hello_metadata.wire_format;
    let server_id = worker_connect.hello_metadata.server_id;

    // If we are using wire encryption then exchange secrets
    let ek = match wire_encryption {
        Some(key_size) => Some(key_exchange::mesh_key_exchange_sender(&mut worker_connect.stream_rx, &mut stream_tx, key_size, validation).await?),
        None => None,
    };

    // background thread - connects and then runs inbox and outbox threads
    // if the upstream object signals a termination event it will exit
    trace!("spawning connect worker");
    TaskEngine::spawn(
        mesh_connect_worker::<M, C>(
            worker_connect,
            addr,
            ek,
            node_id,
            server_id,
            inbox,
            metrics,
            throttle,
            exit,
        )
    );

    trace!("building upstream with tx channel");
    let stream_tx = StreamTxChannel::new(stream_tx, ek);
    Ok(Upstream {
        id: node_id,
        outbox: stream_tx,
        wire_format,
    })
}

struct MeshConnectContext
{
    addr: MeshConnectAddr,
    stream_rx: StreamRx,
    hello_metadata: HelloMetadata,
}

#[allow(unused_variables)]
async fn mesh_connect_prepare
(
    
    addr: MeshConnectAddr,
    hello_path: String,
    node_id: NodeId,
    domain: String,
    wire_protocol: StreamProtocol,
    wire_encryption: Option<KeySize>,
    #[allow(unused_variables)]
    fail_fast: bool,
)
-> Result<(MeshConnectContext, StreamTx), CommsError>
{
    async move {
        #[allow(unused_mut)]
        let mut exp_backoff = Duration::from_millis(100);
        loop {
            #[cfg(feature = "enable_full")]
            let stream = {
                #[cfg(not(feature="enable_dns"))]
                let addr = {
                    match format!("{}:{}", addr.host, addr.port)
                        .to_socket_addrs()?
                        .next()
                    {
                        Some(a) => a,
                        None => {
                            bail!(CommsErrorKind::InvalidDomainName);
                        }
                    }
                };

                let stream = match
                    TcpStream::connect(addr.clone())
                    .await
                {
                    Err(err) if match err.kind() {
                        std::io::ErrorKind::ConnectionRefused => {
                            if fail_fast {
                                bail!(CommsErrorKind::Refused);
                            }
                            true
                        },
                        std::io::ErrorKind::ConnectionReset => true,
                        std::io::ErrorKind::ConnectionAborted => true,
                        _ => false   
                    } => {
                        debug!("connect failed: reason={}, backoff={}s", err, exp_backoff.as_secs_f32());
                        tokio::time::sleep(exp_backoff).await;
                        exp_backoff *= 2;
                        if exp_backoff > Duration::from_secs(10) { exp_backoff = Duration::from_secs(10); }
                        continue;
                    },
                    a => a?,
                };

                // Setup the TCP stream
                setup_tcp_stream(&stream)?;

                // Convert the TCP stream into the right protocol
                let stream = Stream::Tcp(stream);
                let stream = stream
                    .upgrade_client(wire_protocol)
                    .await?;
                stream
            };

            #[cfg(all(feature = "enable_web_sys",not(feature = "enable_full")))]
            let stream = {
                trace!("opening /dev/tok");
                let file = std::fs::File::open("/dev/tok")?;
                Stream::WebSocket(file)
            };

            // Build the stream
            trace!("splitting stream into rx/tx");
            let (mut stream_rx, mut stream_tx) = stream.split();

            // Say hello
            let hello_metadata =
                hello::mesh_hello_exchange_sender(&mut stream_rx, &mut stream_tx, node_id, hello_path.clone(), domain.clone(), wire_encryption)
                .await?;
            
                // Return the result
            return Ok((MeshConnectContext {
                addr,
                stream_rx,
                hello_metadata,
            }, stream_tx));
        }
    }
    .instrument(tracing::info_span!("connect"))
    .await
}

async fn mesh_connect_worker<M, C>
(
    connect: MeshConnectContext,
    sock_addr: MeshConnectAddr,
    wire_encryption: Option<EncryptKey>,
    node_id: NodeId,
    peer_id: NodeId,
    inbox: Box<dyn InboxProcessor<M, C>>,
    metrics: Arc<StdMutex<super::metrics::Metrics>>,
    throttle: Arc<StdMutex<super::throttle::Throttle>>,
    exit: broadcast::Receiver<()>
)
where M: Send + Sync + Serialize + DeserializeOwned + Clone + Default + 'static,
      C: Send + Sync + Default + 'static,
{
    let span = span!(Level::DEBUG, "client", id=node_id.to_short_string().as_str(), peer=peer_id.to_short_string().as_str());
    let wire_format = connect.hello_metadata.wire_format;

    let context = Arc::new(C::default());
    match process_inbox::<M, C>
    (
        connect.stream_rx,
        inbox,
        metrics,
        throttle,
        node_id,
        peer_id,
        sock_addr,
        context,
        wire_format,
        wire_encryption,
        exit
    )
    .instrument(span.clone())
    .await {
        Ok(_) => { },
        Err(CommsError(CommsErrorKind::IO(err), _)) if match err.kind() {
            std::io::ErrorKind::BrokenPipe => true,
            std::io::ErrorKind::UnexpectedEof => true,
            std::io::ErrorKind::ConnectionReset => true,
            std::io::ErrorKind::ConnectionAborted => true,
            _ => false,
        } => { },
        Err(err) => {
            warn!("connection-failed: {}", err.to_string());
        },
    };

    let _span = span.enter();

    //#[cfg(feature = "enable_verbose")]
    debug!("disconnected-inbox: {}", connect.addr.to_string());
}