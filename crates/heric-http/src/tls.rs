//! Trust roots and client certificates.

use std::path::PathBuf;

/// A CA bundle the user added to a workspace.
///
/// An addition to the operating system's store, never a replacement of it: the
/// original sets libcurl's native-CA option unconditionally and passes the
/// workspace bundle alongside (`PLT-024`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CaSource {
    /// PEM bytes, handed to libcurl as a memory blob.
    Pem(Vec<u8>),
    /// A file on disk, read at request time so an edited file takes effect
    /// without a restart.
    Path(PathBuf),
}

/// A client certificate presented to a host.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ClientCertificate {
    /// Host the certificate applies to.
    pub host: String,
    /// Port, when the certificate is port-specific.
    pub port: Option<u16>,
    /// Certificate material.
    pub certificate: CertificateSource,
    /// Key material, when the two are separate.
    pub key: CertificateSource,
    /// Key passphrase, when the key is encrypted.
    ///
    /// Held as a string because libcurl takes one; it never reaches the
    /// timeline or the log.
    pub passphrase: Option<String>,
}

/// Where a certificate or key comes from.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CertificateSource {
    /// PEM bytes.
    Pem(Vec<u8>),
    /// A PKCS#12 archive.
    P12(Vec<u8>),
    /// A file on disk.
    Path(PathBuf),
    /// Nothing supplied; the field is absent in the original too.
    Absent,
}

/// How the engine verifies the server.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TlsConfig {
    /// Use the operating system's trust store. Always true in Heric; the field
    /// exists so a test can prove the default rather than assume it.
    pub native_ca: bool,
    /// Extra roots from the workspace, added to the native store.
    pub workspace_ca: Option<CaSource>,
    /// A certificate to present.
    pub client_certificate: Option<ClientCertificate>,
}

impl Default for TlsConfig {
    /// The native store is always on, so the default is [`TlsConfig::native_only`].
    fn default() -> Self {
        Self::native_only()
    }
}

impl TlsConfig {
    /// The default: native store, no additions.
    #[must_use]
    pub fn native_only() -> Self {
        Self {
            native_ca: true,
            workspace_ca: None,
            client_certificate: None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn native_only_is_the_default() {
        assert_eq!(TlsConfig::default(), TlsConfig::native_only());
        assert!(TlsConfig::default().native_ca);
    }
}
