/* This is part of mktcb - which is under the MIT License ********************/

use snafu::{Snafu};

#[derive(Debug, Snafu)]
#[snafu(visibility(pub))]
pub enum Error {
    #[snafu(display("Failed to initialize logging"))]
    LogInitFailed {
    },

    #[snafu(display("The version must be of format 'X.Y'. Found {}", orig))]
    InvalidVersionFormat {
        orig: String,
    },
    #[snafu(display("Failed to parse version component in '{}': {}", string, source))]
    InvalidVersionNumber {
        source: std::num::ParseIntError,
        string: String,
    },

    #[snafu(display("Cannot retrieve Linux updates because no source has been downloaded (run --fetch?)"))]
    LinuxNotFetched {
    },

    #[snafu(display("The URL to retrieve Linux seems invalid: {}", source))]
    InvalidLinuxURL {
        source: url::ParseError,
    },
    #[snafu(display("The URL to retrieve the toolchain seems invalid: {}", source))]
    InvalidToolchainURL {
        source: url::ParseError,
    },
    #[snafu(display("The URL to retrieve U-Boot seems invalid: {}", source))]
    InvalidUbootURL {
        source: url::ParseError,
    },

    #[snafu(display("Failed to read version file {:#?}: {}", path, source))]
    FailedToReadVersion {
        path: std::path::PathBuf,
        source: std::io::Error,
    },

    #[snafu(display("Failed to decode UTF-8 string {}", source))]
    FailedToDecodeUTF8 {
        source: std::string::FromUtf8Error,
    },

    #[snafu(display("Corrupted download directory: the version file {:#?} does \
            not exist, but the source directory {:#?} exists. Please remove this directory.",
            version_file, dir))]
    CorruptedSourceDir {
        dir: std::path::PathBuf,
        version_file: std::path::PathBuf,
    },

    #[snafu(display("Could not retrieve current directory: {}", source))]
    CwdAccess {
        source: std::io::Error,
    },

    #[snafu(display("Target option (--target, -t) is required"))]
    MissingTarget {
    },

    #[snafu(display("Invalid job number: {}", source))]
    InvalidJobNumber {
        source: std::num::ParseIntError,
    },

    #[snafu(display("A value of 0 jobs is meaningless"))]
    ZeroJob {
    },

    #[snafu(display("Failed to read file {:#?}: {}", path, source))]
    FailedToRead {
        path: std::path::PathBuf,
        source: std::io::Error,
    },

    #[snafu(display("Failed to open file {:#?}: {}", path, source))]
    FailedToOpen {
        path: std::path::PathBuf,
        source: std::io::Error,
    },

    #[snafu(display("Failed to decode Xz data at path {:#?}: {}", path, source))]
    FailedToDecodeXz {
        path: std::path::PathBuf,
        source: std::io::Error,
    },

    #[snafu(display("Failed to read file {:#?}: {}", path, source))]
    FailedToDeser {
        path: std::path::PathBuf,
        source: toml::de::Error,
    },

    #[snafu(display("File {:#?} does not exist", path))]
    FileDoesNotExist {
        path: std::path::PathBuf,
    },

    #[snafu(display("Ill-formed path {:#?}", path))]
    IllFormedPath {
        path: std::path::PathBuf,
    },

    #[snafu(display("Failed to run process '{}': {}", proc, source))]
    ProgFailed {
        source: std::io::Error,
        proc: String,
    },

    #[snafu(display("Failed to decompress {:#?}", path))]
    TarFailed {
        path: std::path::PathBuf,
    },

    #[snafu(display("Failed to apply patch to {:#?}", path))]
    PatchFailed {
        path: std::path::PathBuf,
    },

    #[snafu(display("Archive {:#?} was expected to be decompressed as directory {:#?}", arch, dir))]
    UnexpectedUntar {
        arch: std::path::PathBuf,
        dir: std::path::PathBuf,
    },

    #[snafu(display("Failed to create directory {:?}: {}", path, source))]
    CreateDirError {
        source: std::io::Error,
        path: std::path::PathBuf,
    },

    #[snafu(display("Failed to create/open file {:#?}: {}", path, source))]
    CreateFileError {
        source: std::io::Error,
        path: std::path::PathBuf,
    },

    #[snafu(display("curl refused url '{:#?}': {}", url, source))]
    URLError {
        source: curl::Error,
        url: url::Url,
    },

    #[snafu(display("Failed to setup curl: {}", source))]
    CURLSetupError {
        source: curl::Error,
    },

    #[snafu(display("Failed to write data at path {:#?}: {}", path, source))]
    FailedToWrite {
        source: std::io::Error,
        path: std::path::PathBuf,
    },

    #[snafu(display("Failed to download file from URL {:#?}: HTTP code: {}", url, code))]
    DownloadError {
        code: u32,
        url: url::Url,
    },

    #[snafu(display("Failed to download file from URL {:#?}: {}", url, source))]
    RequestError {
        source: curl::Error,
        url: url::Url,
    },

    #[snafu(display("Failed to setup signal handler: {}", source))]
    CtrlCFailed {
        source: ctrlc::Error,
    },

    #[snafu(display("Failed to iterate over directory {:#?}: {}", dir, source))]
    DirIterFailed {
        dir: std::path::PathBuf,
        source: std::io::Error,
    },

    #[snafu(display("Failed to retrieve the canonical path to {:#?}: {}", dir, source))]
    CanonFailed {
        dir: std::path::PathBuf,
        source: std::io::Error,
    },

    #[snafu(display("Failed to copy {:#?} to {:#?}: {}", from, to, source))]
    CopyFailed {
        from: std::path::PathBuf,
        to: std::path::PathBuf,
        source: std::io::Error,
    },

    #[snafu(display("Failed to run the make target '{}'", target))]
    MakeFailed {
        target: String,
    },

    #[snafu(display("Failed to extract last URL component from {:#?}", url))]
    URLExtractError {
        url: url::Url,
    },

    #[snafu(display("Failed retrieve mandatory environment variable '{}': {}", var, source))]
    MaintainerError {
        source: std::env::VarError,
        var: String,
    },

    #[snafu(display("Failed to create Debian package '{}'", package))]
    DebFailed {
        package: String,
    },

    #[snafu(display("We were expected to have created a Debian package at path {:#?}", path))]
    NoPackage {
        path: std::path::PathBuf,
    },
}
pub type Result<T, E = Error> = std::result::Result<T, E>;
