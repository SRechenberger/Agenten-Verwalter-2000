//! TLS der Fernsteuerung (ADR 059): ein selbst signiertes Zertifikat je Host, das der Gast nur gegen
//! den Fingerabdruck aus der Einladung prüft. Kryptografie über `ring`, nie `aws-lc-rs`.
use std::fmt::Write as _;
use std::fs;
use std::io;
use std::path::Path;
use std::sync::Arc;

use rustls::client::danger::{HandshakeSignatureValid, ServerCertVerified, ServerCertVerifier};
use rustls::crypto::{CryptoProvider, verify_tls12_signature, verify_tls13_signature};
use rustls::pki_types::pem::PemObject;
use rustls::pki_types::{CertificateDer, PrivateKeyDer, ServerName, UnixTime};
use rustls::{
    CertificateError, ClientConfig, DigitallySignedStruct, Error, ServerConfig, SignatureScheme,
};
use sha2::{Digest, Sha256};

use super::write_private;

/// Name im Zertifikat und beim Verbinden; geprüft wird er nicht, nur der Fingerabdruck.
pub const SERVER_NAME: &str = "verwalter";

/// Zertifikat und Schlüssel des Hosts.
#[derive(Debug)]
pub struct HostIdentity {
    pub certificate: CertificateDer<'static>,
    key: PrivateKeyDer<'static>,
    /// SHA-256 des Zertifikats, hex, Kleinbuchstaben.
    pub fingerprint: String,
}

fn provider() -> Arc<CryptoProvider> {
    Arc::new(rustls::crypto::ring::default_provider())
}

/// `N` Zufallsbytes aus derselben Quelle wie TLS.
pub fn random_bytes<const N: usize>() -> io::Result<[u8; N]> {
    let mut bytes = [0_u8; N];
    provider()
        .secure_random
        .fill(&mut bytes)
        .map_err(|_| io::Error::other("Keine Zufallszahlen vom System"))?;
    Ok(bytes)
}

pub fn sha256_hex(bytes: &[u8]) -> String {
    Sha256::digest(bytes)
        .iter()
        .fold(String::with_capacity(64), |mut text: String, byte: &u8| {
            let _ = write!(text, "{byte:02x}");
            text
        })
}

pub fn fingerprint_of(certificate: &CertificateDer<'_>) -> String {
    sha256_hex(certificate.as_ref())
}

/// Lädt Zertifikat und Schlüssel aus `path` oder legt beide an (unter Linux 0600).
pub fn load_or_create(path: &Path) -> io::Result<HostIdentity> {
    match fs::read(path) {
        Ok(pem) => parse_identity(&pem),
        Err(error) if error.kind() == io::ErrorKind::NotFound => {
            let generated = rcgen::generate_simple_self_signed(vec![SERVER_NAME.to_owned()])
                .map_err(io::Error::other)?;
            let pem = format!(
                "{}{}",
                generated.cert.pem(),
                generated.signing_key.serialize_pem()
            );
            write_private(path, pem.as_bytes())?;
            parse_identity(pem.as_bytes())
        }
        Err(error) => Err(error),
    }
}

fn parse_identity(pem: &[u8]) -> io::Result<HostIdentity> {
    let certificate = CertificateDer::from_pem_slice(pem).map_err(io::Error::other)?;
    let key = PrivateKeyDer::from_pem_slice(pem).map_err(io::Error::other)?;
    Ok(HostIdentity {
        fingerprint: fingerprint_of(&certificate),
        certificate,
        key,
    })
}

pub fn server_config(identity: &HostIdentity) -> Result<Arc<ServerConfig>, Error> {
    let mut config = ServerConfig::builder_with_provider(provider())
        .with_safe_default_protocol_versions()?
        .with_no_client_auth()
        .with_single_cert(vec![identity.certificate.clone()], identity.key.clone_key())?;
    // Der Gast nimmt keine Wiederaufnahme an; Tickets wären nur Bytes, die niemand liest.
    config.send_tls13_tickets = 0;
    Ok(Arc::new(config))
}

pub fn client_config(fingerprint: &str) -> Result<Arc<ClientConfig>, Error> {
    let provider = provider();
    let verifier = FingerprintVerifier {
        fingerprint: fingerprint.to_ascii_lowercase(),
        provider: Arc::clone(&provider),
    };
    let mut config = ClientConfig::builder_with_provider(provider)
        .with_safe_default_protocol_versions()?
        .dangerous()
        .with_custom_certificate_verifier(Arc::new(verifier))
        .with_no_client_auth();
    config.resumption = rustls::client::Resumption::disabled();
    Ok(Arc::new(config))
}

/// Ob ein TLS-Fehler sagt, dass das Zertifikat nicht zum Fingerabdruck passt.
pub fn is_fingerprint_mismatch(error: &io::Error) -> bool {
    error
        .get_ref()
        .and_then(|inner| inner.downcast_ref::<Error>())
        .is_some_and(|error: &Error| matches!(error, Error::InvalidCertificate(_)))
}

/// Nimmt genau das Zertifikat mit dem erwarteten Fingerabdruck an; Name, Aussteller und Laufzeit
/// zählen nicht. Die Signaturen des Handschlags prüft `ring` wie sonst auch.
#[derive(Debug)]
struct FingerprintVerifier {
    fingerprint: String,
    provider: Arc<CryptoProvider>,
}

impl ServerCertVerifier for FingerprintVerifier {
    fn verify_server_cert(
        &self,
        end_entity: &CertificateDer<'_>,
        _intermediates: &[CertificateDer<'_>],
        _server_name: &ServerName<'_>,
        _ocsp_response: &[u8],
        _now: UnixTime,
    ) -> Result<ServerCertVerified, Error> {
        if fingerprint_of(end_entity) == self.fingerprint {
            Ok(ServerCertVerified::assertion())
        } else {
            Err(Error::InvalidCertificate(
                CertificateError::ApplicationVerificationFailure,
            ))
        }
    }

    fn verify_tls12_signature(
        &self,
        message: &[u8],
        cert: &CertificateDer<'_>,
        dss: &DigitallySignedStruct,
    ) -> Result<HandshakeSignatureValid, Error> {
        verify_tls12_signature(
            message,
            cert,
            dss,
            &self.provider.signature_verification_algorithms,
        )
    }

    fn verify_tls13_signature(
        &self,
        message: &[u8],
        cert: &CertificateDer<'_>,
        dss: &DigitallySignedStruct,
    ) -> Result<HandshakeSignatureValid, Error> {
        verify_tls13_signature(
            message,
            cert,
            dss,
            &self.provider.signature_verification_algorithms,
        )
    }

    fn supported_verify_schemes(&self) -> Vec<SignatureScheme> {
        self.provider
            .signature_verification_algorithms
            .supported_schemes()
    }
}
