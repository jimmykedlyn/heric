//! The target and client matrix.

/// A language a snippet can be written in.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Target {
    /// `curl` on the command line.
    Curl,
    /// Node.js.
    Node,
    /// Browser JavaScript.
    JavaScript,
    /// Python.
    Python,
    /// Go.
    Go,
    /// Java.
    Java,
    /// C#.
    CSharp,
    /// PHP.
    Php,
    /// Ruby.
    Ruby,
    /// A POSIX shell.
    Shell,
    /// Swift.
    Swift,
    /// Kotlin.
    Kotlin,
    /// Objective-C.
    ObjectiveC,
    /// R.
    R,
    /// OCaml.
    Ocaml,
    /// PowerShell.
    PowerShell,
}

/// A client library within a target.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Client {
    /// `curl`.
    Curl,
    /// The target's built-in HTTP support.
    Native,
    /// A browser or Node `fetch`.
    Fetch,
    /// `axios`.
    Axios,
    /// `XMLHttpRequest`.
    XmlHttpRequest,
    /// Python `requests`.
    Requests,
    /// Python `httpx`.
    Httpx,
    /// Java `OkHttp`.
    OkHttp,
    /// Java `Unirest`.
    Unirest,
    /// C# `RestSharp`.
    RestSharp,
    /// C# `HttpClient`.
    HttpClient,
    /// PHP `curl`.
    PhpCurl,
    /// PHP `Guzzle`.
    Guzzle,
    /// HTTPie.
    Httpie,
    /// `wget`.
    Wget,
    /// `URLSession`.
    UrlSession,
    /// `NSURLSession`.
    NsUrlSession,
    /// R `httr`.
    Httr,
    /// R `httr2`.
    Httr2,
    /// OCaml `cohttp`.
    Cohttp,
}

/// How many targets the snippet modal offers.
pub const TARGET_COUNT: usize = 16;

/// How many clients the snippet modal offers.
pub const CLIENT_COUNT: usize = 20;

impl Target {
    /// The name shown in the modal.
    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Curl => "curl",
            Self::Node => "Node",
            Self::JavaScript => "JavaScript",
            Self::Python => "Python",
            Self::Go => "Go",
            Self::Java => "Java",
            Self::CSharp => "C#",
            Self::Php => "PHP",
            Self::Ruby => "Ruby",
            Self::Shell => "Shell",
            Self::Swift => "Swift",
            Self::Kotlin => "Kotlin",
            Self::ObjectiveC => "Objective-C",
            Self::R => "R",
            Self::Ocaml => "OCaml",
            Self::PowerShell => "PowerShell",
        }
    }

    /// The file extension a snippet would be saved with.
    #[must_use]
    pub fn extension(self) -> &'static str {
        match self {
            Self::Curl | Self::Shell => "sh",
            Self::Node | Self::JavaScript => "js",
            Self::Python => "py",
            Self::Go => "go",
            Self::Java => "java",
            Self::CSharp => "cs",
            Self::Php => "php",
            Self::Ruby => "rb",
            Self::Swift => "swift",
            Self::Kotlin => "kt",
            Self::ObjectiveC => "m",
            Self::R => "r",
            Self::Ocaml => "ml",
            Self::PowerShell => "ps1",
        }
    }
}

impl Client {
    /// The name shown in the modal.
    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Curl => "curl",
            Self::Native => "native",
            Self::Fetch => "fetch",
            Self::Axios => "axios",
            Self::XmlHttpRequest => "XMLHttpRequest",
            Self::Requests => "requests",
            Self::Httpx => "httpx",
            Self::OkHttp => "okhttp",
            Self::Unirest => "unirest",
            Self::RestSharp => "restsharp",
            Self::HttpClient => "httpclient",
            Self::PhpCurl => "curl",
            Self::Guzzle => "guzzle",
            Self::Httpie => "httpie",
            Self::Wget => "wget",
            Self::UrlSession => "urlsession",
            Self::NsUrlSession => "nsurlsession",
            Self::Httr => "httr",
            Self::Httr2 => "httr2",
            Self::Cohttp => "cohttp",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn target_count_matches_the_enum() {
        // Keep the constant honest: adding a variant means updating the count.
        let all = [
            Target::Curl,
            Target::Node,
            Target::JavaScript,
            Target::Python,
            Target::Go,
            Target::Java,
            Target::CSharp,
            Target::Php,
            Target::Ruby,
            Target::Shell,
            Target::Swift,
            Target::Kotlin,
            Target::ObjectiveC,
            Target::R,
            Target::Ocaml,
            Target::PowerShell,
        ];
        assert_eq!(all.len(), TARGET_COUNT);
    }

    #[test]
    fn client_count_matches_the_enum() {
        let all = [
            Client::Curl,
            Client::Native,
            Client::Fetch,
            Client::Axios,
            Client::XmlHttpRequest,
            Client::Requests,
            Client::Httpx,
            Client::OkHttp,
            Client::Unirest,
            Client::RestSharp,
            Client::HttpClient,
            Client::PhpCurl,
            Client::Guzzle,
            Client::Httpie,
            Client::Wget,
            Client::UrlSession,
            Client::NsUrlSession,
            Client::Httr,
            Client::Httr2,
            Client::Cohttp,
        ];
        assert_eq!(all.len(), CLIENT_COUNT);
    }

    #[test]
    fn extensions_are_distinct_per_language_family() {
        assert_eq!(Target::Curl.extension(), "sh");
        assert_eq!(Target::Python.extension(), "py");
        assert_eq!(Target::PowerShell.extension(), "ps1");
    }
}
