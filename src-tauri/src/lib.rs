pub mod commands;
pub mod crypto;
pub mod error;
pub mod metadata;
pub mod platform;
pub mod storage;
pub mod vault;

pub use error::{Error, Result};

pub fn run() {
    commands::run();
}
