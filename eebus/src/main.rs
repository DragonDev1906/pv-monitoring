use std::net::TcpStream;
use std::sync::Arc;

use rustls::{
    ClientConfig, ClientConnection, DigitallySignedStruct, SignatureScheme, StreamOwned,
    client::danger::{HandshakeSignatureValid, ServerCertVerified, ServerCertVerifier},
};
use rustls_pki_types::{CertificateDer, ServerName, UnixTime, PrivateKeyDer, pem::PemObject};
use tungstenite::client::ClientRequestBuilder;
use tungstenite::protocol::Message;
use tungstenite::Bytes;
use serde::{Serialize, Deserialize};

#[derive(Debug)]
struct NoServerCert;

impl ServerCertVerifier for NoServerCert {
    fn verify_server_cert(
        &self,
        end_entity: &CertificateDer<'_>,
        intermediates: &[CertificateDer<'_>],
        server_name: &ServerName<'_>,
        ocsp_response: &[u8],
        now: UnixTime,
    ) -> Result<ServerCertVerified, rustls::Error> {
        Ok(ServerCertVerified::assertion())
    }
    fn verify_tls12_signature(
        &self,
        message: &[u8],
        cert: &CertificateDer<'_>,
        dss: &DigitallySignedStruct,
    ) -> Result<HandshakeSignatureValid, rustls::Error> {
        Ok(HandshakeSignatureValid::assertion())
    }
    fn verify_tls13_signature(
        &self,
        message: &[u8],
        cert: &CertificateDer<'_>,
        dss: &DigitallySignedStruct,
    ) -> Result<HandshakeSignatureValid, rustls::Error> {
        Ok(HandshakeSignatureValid::assertion())
    }
    fn supported_verify_schemes(&self) -> Vec<SignatureScheme> {
        vec![
            SignatureScheme::ECDSA_NISTP256_SHA256,
            SignatureScheme::RSA_PKCS1_SHA256,
            SignatureScheme::RSA_PSS_SHA256,
        ]
    }
}

fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt::init();

    let mut args = std::env::args();
    let _ = args.next().unwrap();
    let addr = args.next().unwrap(); // wss://<ip>:12480
    assert_eq!(args.next(), None);

    // eebus requires mutual authentication
    // openssl req -newkey ec -pkeyopt ec_paramgen_curve:P-256 -nodes -x509 -days 3650 -sha256 -keyout client.key -out client.crt
    let key = PrivateKeyDer::from_pem_file("client.key")?;
    let cert = CertificateDer::from_pem_file("client.crt")?;

    let mut cfg = rustls::ClientConfig::builder()
        .with_root_certificates(rustls::RootCertStore::empty())
        .with_client_auth_cert(vec![cert], key)?;
    cfg.dangerous()
        .set_certificate_verifier(Arc::new(NoServerCert));

    let no_schema = match addr.split_once("://") {
        None => &addr,
        Some(x) => x.1,
    };
    let ip = no_schema.split_once(':').unwrap().0;
    dbg!(no_schema, ip);

    let tcp = TcpStream::connect(&no_schema)?;
    let server_name = ServerName::IpAddress(ip.try_into()?);
    let tls = ClientConnection::new(Arc::new(cfg), server_name)?;
    let tls = StreamOwned::new(tls, tcp);

    // https://github.com/DerAndereAndi/eebus-rust/blob/dev/src/main.rs#L391
    let ws_key = tungstenite::handshake::client::generate_key();
    let req = ClientRequestBuilder::new(format!("{addr}/ship/").parse()?)
        // .with_header("Sec-WebSocket-Version", "13")
        // .with_header("Sec-WebSocket-Protocol", "ship")
        .with_sub_protocol("ship");



    let (mut socket, res) = tungstenite::client(req, tls)?;
    dbg!(&socket, res);

    // Ship is json over websocket (except for the first messages)
    // https://deepwiki.com/enbility/ship-go/6.2-handshake-process

    // Ship Handshake 1: CMI_STATE_CLIENT_SEND
    // https://github.com/enbility/ship-go/blob/dev/ship/hs_init.go#L12
    socket.write(Message::Binary(Bytes::from_static(&[0,0])))?; // MsgTypeInit + 0x00
    socket.flush()?;

    let res = socket.read()?; // CMI Init
    dbg!(&res);
    assert_eq!(res, Message::Binary(Bytes::from_static(&[0,0])));

    // SmeHelloState
    // SmeHelloStateReadyInit
    // SME_HELLO_STATE_READY_INIT
    // Basically json, except that an object is represented as an array of objects with one
    // entry per key and for some reason the outer-most is not an array.
    // See https://github.com/enbility/ship-go/blob/dev/ship/helper.go#L55
    let msg = concat!(
        "\x01", // MessageType::Control
        r#"{"connectionHello":[{"phase":"ready"},{"waiting":60000}]}"#
    ).as_bytes();
    socket.write(Message::Binary(msg.into()))?;
    socket.flush()?;

    // SME_HELLO_STATE_READY_LISTEN
    // TODO: Check message content instead of assuming it is the expected value.
    // Expecting something like:
    // > {"connectionHello":[{"phase":"ready"},{"waiting":60000}]}
    let res = socket.read()?;
    dbg!(&res);

    // SmeHelloStateOk
    // SmeProtHStateClientInit
    let msg = concat!(
        "\x01",
        r#"{"messageProtocolHandshake":[{"handshakeType":"announceMax"},{"version":[{"major":1},{"minor":0}]},{"formats":[{"format":["JSON-UTF8"]}]}]}"#
    ).as_bytes();
    socket.write(Message::Binary(msg.into()))?;
    socket.flush()?;

    // SmeProtHStateClientListenChoice
    // TODO: Check message content instead of assuming it is the expected value.
    // Expecting something like:
    // > {"messageProtocolHandshake":[{"handshakeType":"select"},{"version":[{"major":1},{"minor":0}]},{"formats":[{"format":["JSON-UTF8"]}]}]}
    let res = socket.read()?;
    dbg!(&res);

    // Apparently now we acknowledge, too instead of the server just choosing what he wanted.
    let msg = concat!(
        "\x01",
        r#"{"messageProtocolHandshake":[{"handshakeType":"select"},{"version":[{"major":1},{"minor":0}]},{"formats":[{"format":["JSON-UTF8"]}]}]}"#
    ).as_bytes();
    socket.write(Message::Binary(msg.into()))?;
    socket.flush()?;

    // SmeProtHStateClientOk
    // SmePinStateCheckInit
    // smePinState
    // handshakePin_Init
    let msg = concat!(
        "\x01",
        r#"{"connectionPinState":[{"pinState":"none"}]}"#
    ).as_bytes();
    socket.write(Message::Binary(msg.into()))?;
    socket.flush()?;

    // SmePinStateCheckListen
    // handshakePin_smePinStateCheckListen
    // TODO: Check message content instead of assuming it is the expected value.
    // Expecting something like:
    // > {"connectionPinState":[{"pinState":"none"}]}
    let res = socket.read()?;
    dbg!(&res);

    // SmePinStateCheckOk
    // handshakeAccessMethods_Init
    // NOTE: Looks like both client and server are in this state, the order doesn't seem to matter.
    // We send first and receive later, that way we can't deadlock if the server does the same.
    let msg = concat!(
        "\x01",
        r#"{"accessMethodsRequest":[]}"# // Apparently this is just an empty struct.
    ).as_bytes();
    socket.write(Message::Binary(msg.into()))?;
    socket.flush()?;

    // SmeAccessMethodsRequest
    // handshakeAccessMethods_Request
    // TODO: Check message content instead of assuming it is the expected value.
    // Expecting something like:
    // > {"accessMethodsRequest":[]}
    let x = socket.read()?;
    dbg!(x);

    // handleAccessMethodsRequest - "request"
    // TODO: May not want a hard-coded id.
    let msg = concat!(
        "\x01",
        r#"{"accessMethods":[{"id":"MONITORING"}]}"# // Apparently this is just an empty struct.
    ).as_bytes();
    socket.write(Message::Binary(msg.into()))?;
    socket.flush()?;
    
    // handshakeAccessMethods_Request - "methods"
    // TODO: Check message content instead of assuming it is the expected value.
    // Expecting something like:
    // > {"accessMethods":[{"id":"…"},{"dnsSd_mDns":[]}]}
    let x = socket.read()?;
    dbg!(x);

    // SmeStateApproved
    // approveHandshake


    Ok(())
}

#[derive(Serialize, Deserialize, Default, Debug)]
#[serde(rename_all = "camelCase")]
pub struct ConnectionHello {
  pub connection_hello: ConnectionHelloType,
}

#[derive(Serialize, Deserialize, Default, Debug)]
#[serde(rename_all = "camelCase")]
pub struct ConnectionHelloType {
  #[serde(skip_serializing_if = "Option::is_none")]
  pub phase: Option<ConnectionHelloPhaseType>,
  #[serde(skip_serializing_if = "Option::is_none")]
  pub waiting: Option<u32>,
  #[serde(skip_serializing_if = "Option::is_none")]
  pub prolongation_request: Option<bool>,
}

#[derive(Serialize, Deserialize, Debug, Clone, Copy, Hash, PartialEq, Eq)]
pub enum ConnectionHelloPhaseType {
	#[serde(rename = "pending")]
  Pending,
	#[serde(rename = "ready")]
  Ready,
	#[serde(rename = "aborted")]
  Aborted,
}
