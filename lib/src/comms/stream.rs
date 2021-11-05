#![allow(unused_imports)]
use tracing::{info, warn, debug, error, trace, instrument, span, Level};
use error_chain::bail;
#[cfg(feature = "enable_full")]
use tokio::net::TcpStream;
#[cfg(feature = "enable_full")]
use tokio::net::tcp::OwnedReadHalf;
#[cfg(feature = "enable_full")]
use tokio::net::tcp::OwnedWriteHalf;
use tokio::io::{AsyncRead, AsyncWrite};
use std::str::FromStr;
use tokio::time::timeout as tokio_timeout;
use std::time::Duration;
use std::result::Result;
use std::sync::Arc;
use std::io::{Read, Write};
use std::fs::File;

use crate::crypto::EncryptKey;
use crate::comms::PacketData;

#[cfg(feature = "enable_server")]
use
{
    hyper_tungstenite     :: hyper::upgrade::Upgraded as HyperUpgraded,
    hyper_tungstenite     :: tungstenite::Message as HyperMessage,
    hyper_tungstenite     :: WebSocketStream as HyperWebSocket,
    hyper_tungstenite     :: tungstenite::Error as HyperError,
    tokio_tungstenite     :: { tungstenite::{ Message }, WebSocketStream    },
    tokio                 :: io::{ AsyncWriteExt, AsyncReadExt },
};

use crate::error::*;

#[derive(Debug, Clone, Copy)]
pub enum StreamProtocol
{
    Tcp,
    WebSocket,
}

impl std::str::FromStr
for StreamProtocol
{
    type Err = CommsError;

    fn from_str(s: &str) -> Result<StreamProtocol, CommsError>
    {
        let ret = match s {
            "tcp" => StreamProtocol::Tcp,
            "ws" => StreamProtocol::WebSocket,
            _ => {
                bail!(CommsErrorKind::UnsupportedProtocolError(s.to_string()));
            }
        };
        Ok(ret)
    }
}

impl StreamProtocol
{
    pub fn to_scheme(&self) -> String
    {
        let ret = match self {
            StreamProtocol::Tcp => "tcp",
            StreamProtocol::WebSocket => "ws",
        };
        ret.to_string()
    }

    pub fn to_string(&self) -> String
    {
        self.to_scheme()
    }

    pub fn default_port(&self) -> u16 {
        match self {
            StreamProtocol::Tcp => 5000,
            StreamProtocol::WebSocket => 80,
        }
    }

    pub fn is_tcp(&self) -> bool {
        match self {
            StreamProtocol::Tcp => true,
            StreamProtocol::WebSocket => false,
        }
    }

    pub fn is_web_socket(&self) -> bool {
        match self {
            StreamProtocol::Tcp => false,
            StreamProtocol::WebSocket => true,
        }
    }
}

impl std::fmt::Display
for StreamProtocol
{
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.to_scheme())
    }
}

pub trait AsyncStream : AsyncRead + AsyncWrite + std::fmt::Debug
{
}

#[derive(Debug)]
pub enum Stream
{
    #[cfg(feature = "enable_full")]
    Tcp(TcpStream),
    #[cfg(feature = "enable_full")]
    WebSocket(WebSocketStream<TcpStream>, StreamProtocol),
    #[cfg(feature = "enable_server")]
    HyperWebSocket(HyperWebSocket<HyperUpgraded>, StreamProtocol),
    Custom(Box<dyn AsyncStream>, StreamProtocol),
}

impl StreamProtocol
{
    pub fn make_url(&self, domain: String, port: u16, path: String) -> Result<url::Url, url::ParseError>
    {
        let scheme = self.to_scheme();
        let input = match port {
            a if a == self.default_port() => match path.starts_with("/") {
                true => format!("{}://{}:{}{}", scheme, domain, port, path),
                false => format!("{}://{}:{}/{}", scheme, domain, port, path),
            },
            _ => match path.starts_with("/") {
                true => format!("{}://{}{}", scheme, domain, path),
                false => format!("{}://{}/{}", scheme, domain, path),
            }
        };
        url::Url::parse(input.as_str())
    }

    pub fn parse(url: &url::Url) -> Result<StreamProtocol, CommsError>
    {
        let scheme = url.scheme().to_string().to_lowercase();
        StreamProtocol::from_str(scheme.as_str())
    }
}

#[derive(Debug)]
pub enum StreamRx
{
    #[cfg(feature = "enable_full")]
    Tcp(OwnedReadHalf),
    #[cfg(feature = "enable_full")]
    WebSocket(futures_util::stream::SplitStream<WebSocketStream<TcpStream>>),
    #[cfg(feature = "enable_server")]
    HyperWebSocket(futures_util::stream::SplitStream<HyperWebSocket<HyperUpgraded>>),
    Custom(tokio::io::ReadHalf<Box<dyn AsyncStream>>, StreamProtocol),
}

#[derive(Debug)]
pub enum StreamTx
{
    #[cfg(feature = "enable_full")]
    Tcp(OwnedWriteHalf),
    #[cfg(feature = "enable_full")]
    WebSocket(futures_util::stream::SplitSink<WebSocketStream<TcpStream>, Message>),
    #[cfg(feature = "enable_server")]
    HyperWebSocket(futures_util::stream::SplitSink<HyperWebSocket<HyperUpgraded>, HyperMessage>),
    Custom(tokio::io::WriteHalf<Box<dyn AsyncStream>>, StreamProtocol),
}

impl Stream
{
    pub fn split(self) -> (StreamRx, StreamTx) {
        match self {
            #[cfg(feature = "enable_full")]
            Stream::Tcp(a) => {
                let (rx, tx) = a.into_split();
                (StreamRx::Tcp(rx), StreamTx::Tcp(tx))
            },
            Stream::WebSocket(a) => {
                let (tx, rx) = a.split();
                (StreamRx::WebSocket(rx), StreamTx::WebSocket(tx))
            }
            #[cfg(feature = "enable_server")]
            Stream::HyperWebSocket(a, _) => {
                let (tx, rx) = a.split();
                (StreamRx::HyperWebSocket(rx), StreamTx::HyperWebSocket(tx))
            }
            Stream::Custom(a, p) => {
                use tokio::io::*;
                let (tx, rx) = a.split();
                (StreamRx::Custom(rx, p), StreamTx::Custom(tx, p))
            }
        }
    }

    #[cfg(feature = "enable_server")]
    pub async fn upgrade_server(self, protocol: StreamProtocol, timeout: Duration) -> Result<Stream, CommsError> {
        debug!("tcp-protocol-upgrade(server): {}", protocol);

        let ret = match self {
            #[cfg(feature = "enable_full")]
            Stream::Tcp(a) => {
                match protocol {
                    StreamProtocol::Tcp => {
                        Stream::Tcp(a)
                    },
                    StreamProtocol::WebSocket => {
                        let wait = tokio_tungstenite::accept_async(a);
                        let socket = tokio_timeout(timeout, wait).await??;
                        Stream::WebSocket(socket, protocol)
                    },
                }
            },
            #[cfg(feature = "enable_full")]
            Stream::WebSocket(a, p) => {
                match protocol {
                    StreamProtocol::Tcp => {
                        Stream::WebSocket(a, p)
                    },
                    StreamProtocol::WebSocket => {
                        Stream::WebSocket(a, p)
                    },
                }
            },
            #[cfg(feature = "enable_server")]
            Stream::HyperWebSocket(a, p) => {
                match protocol {
                    StreamProtocol::Tcp => {
                        Stream::HyperWebSocket(a, p)
                    },
                    StreamProtocol::WebSocket => {
                        Stream::HyperWebSocket(a, p)
                    }
                }
            },
            Stream::Custom(a, p) => {
                match protocol {
                    StreamProtocol::Tcp => {
                        Stream::Custom(a, p)
                    },
                    StreamProtocol::WebSocket => {
                        Stream::Custom(a, p)
                    }
                }
            }
        };

        Ok(ret)
    }

    #[allow(dead_code)]
    #[allow(unused_variables)]
    pub async fn upgrade_client(self, protocol: StreamProtocol) -> Result<Stream, CommsError> {
        debug!("tcp-protocol-upgrade(client): {}", protocol);

        let ret = match self {
            #[cfg(feature = "enable_full")]
            Stream::Tcp(a) => {
                match protocol {
                    StreamProtocol::Tcp => Stream::Tcp(a),
                    StreamProtocol::WebSocket => {
                        let url = StreamProtocol::WebSocket.make_url("localhost".to_string(), 80, "/".to_string())?;
                        let mut request = tokio_tungstenite::tungstenite::http::Request::new(());
                        *request.uri_mut() = tokio_tungstenite::tungstenite::http::Uri::from_str(url.as_str())?;
                        let (stream, response) = tokio_tungstenite::client_async(request, a)
                            .await?;
                        if response.status().is_client_error() {
                            bail!(CommsErrorKind::WebSocketInternalError(format!("HTTP error while performing WebSocket handshack - status-code={}", response.status().as_u16())));
                        }
                        Stream::WebSocket(stream, protocol)
                    },
                }
            },
            #[cfg(feature = "enable_full")]
            Stream::WebSocket(a, p) => {
                match protocol {
                    StreamProtocol::Tcp => Stream::WebSocket(a, p),
                    StreamProtocol::WebSocket => Stream::WebSocket(a, p),
                }
            },
            #[cfg(feature = "enable_server")]
            Stream::HyperWebSocket(a, p) => {
                match protocol {
                    StreamProtocol::Tcp => Stream::HyperWebSocket(a, p),
                    StreamProtocol::WebSocket => Stream::HyperWebSocket(a, p)
                }
            },
            Stream::Custom(a, p) => {
                match protocol {
                    StreamProtocol::Tcp => Stream::WebSocket(a),
                    StreamProtocol::WebSocket => Stream::WebSocket(a),
                }
            }
        };
        Ok(ret)
    }

    #[allow(dead_code)]
    pub fn protocol(&self) -> StreamProtocol
    {
        match self {
            #[cfg(feature = "enable_full")]
            Stream::Tcp(_) => StreamProtocol::Tcp,
            #[cfg(feature = "enable_full")]
            Stream::WebSocket(_, p) => p.clone(),
            #[cfg(feature = "enable_server")]
            Stream::HyperWebSocket(_, p) => p.clone(),
            Stream::Custom(_, p) => p,
        }
    }
}

impl StreamTx
{
    #[must_use="all network communication metrics must be accounted for"]
    #[allow(unused_variables)]
    pub async fn write_8bit(&mut self, buf: &'_[u8], delay_flush: bool) -> Result<u64, tokio::io::Error>
    {
        #[allow(unused_mut)]
        let mut total_sent = 0u64;
        match self {
            #[cfg(feature = "enable_full")]
            StreamTx::Tcp(a) => {
                if buf.len() > u8::MAX as usize {
                    return Err(tokio::io::Error::new(tokio::io::ErrorKind::InvalidData, format!("Data is to big to write (len={}, max={})", buf.len(), u8::MAX)));
                }
                a.write_u8(buf.len() as u8).await?;
                total_sent += 1u64;
                a.write_all(&buf[..]).await?; 
                total_sent += buf.len() as u64;
            },
            #[cfg(feature = "enable_full")]
            StreamTx::WebSocket(_) => {
                total_sent += self.write_32bit(buf, delay_flush).await?;
            },
            #[cfg(feature = "enable_server")]
            StreamTx::HyperWebSocket(_) => {
                total_sent += self.write_32bit(buf, delay_flush).await?;
            },
            StreamTx::Custom(file) => {
                total_sent += self.write_32bit(buf, delay_flush).await?;
            },
        }
        #[allow(unreachable_code)]
        Ok(total_sent)
    }

    #[must_use="all network communication metrics must be accounted for"]
    #[allow(unused_variables)]
    pub async fn write_16bit(&mut self, buf: &'_ [u8], delay_flush: bool) -> Result<u64, tokio::io::Error>
    {
        #[allow(unused_mut)]
        let mut total_sent = 0u64;
        match self {
            #[cfg(feature = "enable_full")]
            StreamTx::Tcp(a) => {
                if buf.len() > u16::MAX as usize {
                    return Err(tokio::io::Error::new(tokio::io::ErrorKind::InvalidData, format!("Data is to big to write (len={}, max={})", buf.len(), u16::MAX)));
                }
                a.write_u16(buf.len() as u16).await?;
                total_sent += 2u64;
                a.write_all(&buf[..]).await?; 
                total_sent += buf.len() as u64;
            },
            #[cfg(feature = "enable_full")]
            StreamTx::WebSocket(_) => {
                total_sent += self.write_32bit(buf, delay_flush).await?;
            },
            #[cfg(feature = "enable_server")]
            StreamTx::HyperWebSocket(_) => {
                total_sent += self.write_32bit(buf, delay_flush).await?;
            },
            StreamTx::Custom(_) => {
                total_sent += self.write_32bit(buf, delay_flush).await?;
            }
        }
        #[allow(unreachable_code)]
        Ok(total_sent)
    }

    #[must_use="all network communication metrics must be accounted for"]
    #[allow(unused_variables)]
    pub async fn write_32bit(&mut self, buf: &'_[u8], delay_flush: bool) -> Result<u64, tokio::io::Error>
    {
        #[allow(unused_mut)]
        let mut total_sent = 0u64;
        match self {
            #[cfg(feature = "enable_full")]
            StreamTx::Tcp(a) => {
                if buf.len() > u32::MAX as usize {
                    return Err(tokio::io::Error::new(tokio::io::ErrorKind::InvalidData, format!("Data is to big to write (len={}, max={})", buf.len(), u32::MAX)));
                }
                a.write_u32(buf.len() as u32).await?;
                total_sent += 4u64;
                a.write_all(&buf[..]).await?; 
                total_sent += buf.len() as u64;
            },
            #[cfg(feature = "enable_full")]
            StreamTx::WebSocket(a) => {
                total_sent += buf.len() as u64;
                if delay_flush {
                    match a.feed(Message::binary(buf)).await {
                        Ok(a) => a,
                        Err(err) => {
                            let kind = StreamTx::conv_error_kind(&err);
                            return Err(tokio::io::Error::new(kind, format!("Failed to feed data into websocket - {}", err.to_string())));
                        }
                    }
                } else {
                    match a.send(Message::binary(buf)).await {
                        Ok(a) => a,
                        Err(err) => {
                            let kind = StreamTx::conv_error_kind(&err);
                            return Err(tokio::io::Error::new(kind, format!("Failed to feed data into websocket - {}", err.to_string())));
                        }
                    }
                }
            },
            #[cfg(feature = "enable_server")]
            StreamTx::HyperWebSocket(a) => {
                total_sent += buf.len() as u64;
                if delay_flush {
                    match a.feed(HyperMessage::binary(buf)).await {
                        Ok(a) => a,
                        Err(err) => {
                            let kind = StreamTx::conv_error_kind(&err);
                            return Err(tokio::io::Error::new(kind, format!("Failed to feed data into websocket - {}", err.to_string())));
                        }
                    }
                } else {
                    match a.send(HyperMessage::binary(buf)).await {
                        Ok(a) => a,
                        Err(err) => {
                            let kind = StreamTx::conv_error_kind(&err);
                            return Err(tokio::io::Error::new(kind, format!("Failed to feed data into websocket - {}", err.to_string())));
                        }
                    }
                }
            },
            StreamTx::Custom(a) => {
                if buf.len() > u32::MAX as usize {
                    return Err(tokio::io::Error::new(tokio::io::ErrorKind::InvalidData, format!("Data is to big to write (len={}, max={})", buf.len(), u32::MAX)));
                }
                a.write_all(&buf[..]).await?;
                total_sent += buf.len() as u64;
            }
        }
        #[allow(unreachable_code)]
        Ok(total_sent)
    }

    #[cfg(feature = "enable_server")]
    fn conv_error_kind(err: &HyperError) -> tokio::io::ErrorKind
    {
        match err {
            HyperError::AlreadyClosed => tokio::io::ErrorKind::ConnectionAborted,
            HyperError::ConnectionClosed => tokio::io::ErrorKind::ConnectionAborted,
            HyperError::Io(io) => io.kind(),
            _ => tokio::io::ErrorKind::Other,
        }
    }

    #[must_use="all network communication metrics must be accounted for"]
    pub(crate) async fn send(&mut self, wire_encryption: &Option<EncryptKey>, pck: PacketData)
    -> Result<u64, tokio::io::Error>
    {
        #[allow(unused_mut)]
        let mut total_sent = 0u64;
        match wire_encryption {
            Some(key) => {
                let enc = key.encrypt(&pck.bytes[..]);
                total_sent += self.write_8bit(&enc.iv.bytes, true).await?;
                total_sent += self.write_32bit(&enc.data, false).await?;
            },
            None => {
                total_sent += self.write_32bit(&pck.bytes[..], false).await?;
            }
        };
        #[allow(unreachable_code)]
        Ok(total_sent)
    }
}

#[derive(Debug)]
pub struct StreamTxChannel
{
    tx: StreamTx,
    pub(crate) wire_encryption: Option<EncryptKey>,
}

impl StreamTxChannel
{
    pub fn new(tx: StreamTx, wire_encryption: Option<EncryptKey>) -> StreamTxChannel
    {
        StreamTxChannel {
            tx,
            wire_encryption
        }
    }

    #[must_use="all network communication metrics must be accounted for"]
    pub(crate) async fn send(&mut self, pck: PacketData)
    -> Result<u64, tokio::io::Error>
    {
        self.tx.send(&self.wire_encryption, pck).await
    }
}

impl StreamRx
{
    pub async fn read_8bit(&mut self) -> Result<Vec<u8>, tokio::io::Error>
    {
        #[allow(unused_variables)]
        let ret = match self {
            #[cfg(feature = "enable_full")]
            StreamRx::Tcp(a) => {
                let len = a.read_u8().await?;
                if len <= 0 { return Ok(vec![]); }
                let mut bytes = vec![0 as u8; len as usize];
                let n = a.read_exact(&mut bytes).await?;
                if n != (len as usize) { return Ok(vec![]); }
                bytes
            },
            #[cfg(feature = "enable_full")]
            StreamRx::WebSocket(_) => {
                self.read_32bit().await?
            },
            #[cfg(feature = "enable_server")]
            StreamRx::HyperWebSocket(_) => {
                self.read_32bit().await?
            }
            StreamRx::Custom(_, _) => {
                self.read_32bit().await?
            },
        };
        #[allow(unreachable_code)]
        Ok(ret)
    }

    pub async fn read_16bit(&mut self) -> Result<Vec<u8>, tokio::io::Error>
    {
        #[allow(unused_variables)]
        let ret = match self {
            #[cfg(feature = "enable_full")]
            StreamRx::Tcp(a) => {
                let len = a.read_u16().await?;
                if len <= 0 { return Ok(vec![]); }
                let mut bytes = vec![0 as u8; len as usize];
                let n = a.read_exact(&mut bytes).await?;
                if n != (len as usize) { return Ok(vec![]); }
                bytes
            },
            #[cfg(feature = "enable_full")]
            StreamRx::WebSocket(_) => {
                self.read_32bit().await?
            },
            #[cfg(feature = "enable_server")]
            StreamRx::HyperWebSocket(_) => {
                self.read_32bit().await?
            },
            StreamRx::Custom(_, _) => {
                self.read_32bit().await?
            },
        };
        #[allow(unreachable_code)]
        Ok(ret)
    }

    pub async fn read_32bit(&mut self) -> Result<Vec<u8>, tokio::io::Error>
    {
        #[allow(unused_variables)]
        let ret = match self {
            #[cfg(feature = "enable_full")]
            StreamRx::Tcp(a) => {
                let len = a.read_u32().await?;
                if len <= 0 { return Ok(vec![]); }
                let mut bytes = vec![0 as u8; len as usize];
                let n = a.read_exact(&mut bytes).await?;
                if n != (len as usize) { return Ok(vec![]); }
                bytes
            },
            #[cfg(feature = "enable_full")]
            StreamRx::WebSocket(a) => {
                match a.next().await {
                    Some(a) => {
                        let msg = match a {
                            Ok(a) => a,
                            Err(err) => {
                                return Err(tokio::io::Error::new(tokio::io::ErrorKind::BrokenPipe, format!("Failed to receive data from websocket - {}", err.to_string())));
                            }
                        };
                        match msg {
                            Message::Binary(a) => a,
                            _ => {
                                return Err(tokio::io::Error::new(tokio::io::ErrorKind::BrokenPipe, format!("Failed to receive data from websocket as the message was the wrong type")));
                            }
                        }
                    },
                    None => {
                        return Err(tokio::io::Error::new(tokio::io::ErrorKind::BrokenPipe, format!("Failed to receive data from websocket")));
                    }
                }
            },
            #[cfg(feature = "enable_server")]
            StreamRx::HyperWebSocket(a) => {
                match a.next().await {
                    Some(a) => {
                        let msg = match a {
                            Ok(a) => a,
                            Err(err) => {
                                return Err(tokio::io::Error::new(tokio::io::ErrorKind::BrokenPipe, format!("Failed to receive data from websocket - {}", err.to_string())));
                            }
                        };
                        match msg {
                            HyperMessage::Binary(a) => a,
                            _ => {
                                return Err(tokio::io::Error::new(tokio::io::ErrorKind::BrokenPipe, format!("Failed to receive data from websocket as the message was the wrong type")));
                            }
                        }
                    },
                    None => {
                        return Err(tokio::io::Error::new(tokio::io::ErrorKind::BrokenPipe, format!("Failed to receive data from websocket")));
                    }
                }
            },
            StreamRx::Custom(a, _) => {
                let mut ret = bytes::BytesMut::new();
                loop {
                    let mut buf = [0u8; 16384];
                    let n = a.read(&mut buf).await?;
                    if n > 0 {
                        ret.extend_from_slice(&buf[..n]);
                    } else {
                        break;
                    }
                }
                ret.to_vec()
            },
        };
        #[allow(unreachable_code)]
        Ok(ret)
    }

    #[allow(dead_code)]
    pub fn protocol(&self) -> StreamProtocol
    {
        match self {
            #[cfg(feature = "enable_full")]
            StreamRx::Tcp(_) => StreamProtocol::Tcp,
            #[cfg(feature = "enable_full")]
            StreamRx::WebSocket(_) => StreamProtocol::WebSocket,
            #[cfg(feature = "enable_server")]
            StreamRx::HyperWebSocket(_) => StreamProtocol::WebSocket,
            StreamRx::Custom(_, p) => p,
        }
    }
}