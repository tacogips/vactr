//! Package tests (SS-PKG required tests). No test reaches the network:
//! stores are a local directory, a loopback HTTP fixture and local git
//! repositories under a temp dir.

mod http_fixture;
mod support;

mod cache;
mod digest;
mod driver;
mod load;
mod lock;
mod manifest;
mod mem_cache;
mod mvs;
mod proxy;
mod semver;
mod sha256;
mod stores;
mod validate;
mod zip;
