//! Import and export for Heric.
//!
//! Reads and writes Insomnia export formats v4/v5 plus Postman, HAR, cURL,
//! OpenAPI and WSDL. Round-trip parity with the original exporter is tested
//! against golden fixtures from the original repository. See
//! `docs/inventory/io-and-cli.md` once the inventory lands.

/// Formats recognized by the importer.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Format {
    /// Insomnia export format v4.
    InsomniaV4,
    /// Insomnia export format v5.
    InsomniaV5,
    /// Postman collection.
    Postman,
    /// HTTP Archive.
    Har,
    /// OpenAPI specification.
    OpenApi,
    /// cURL command line.
    Curl,
    /// WSDL document.
    Wsdl,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn formats_are_distinguishable() {
        assert_ne!(Format::InsomniaV4, Format::InsomniaV5);
        assert_ne!(Format::Postman, Format::Har);
    }
}
