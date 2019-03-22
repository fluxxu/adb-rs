#[macro_use]
extern crate log;
#[macro_use]
extern crate num_derive;

pub const VERSION: u32 = 0x01000000;
pub const MAX_DATA: u32 = 0x100000;

pub mod result;

mod message;
mod utils;

mod client;
mod sync;

pub mod push;
pub mod shell;

pub use self::client::{AdbClient, AdbConnection};
pub use self::message::{Command, Header};
