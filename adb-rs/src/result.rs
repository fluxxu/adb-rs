use failure_derive::Fail;

#[derive(Debug, Fail)]
pub enum AdbError {
  #[fail(display = "io error: {}", _0)]
  Io(#[cause] ::std::io::Error),

  #[fail(display = "data crc mismatch")]
  Crc,

  #[fail(display = "auth not supported")]
  AuthNotSupported,

  #[fail(display = "unknown command: {:x}", _0)]
  UnknownCommand(u32),

  #[fail(display = "unexpected command: {:?}", _0)]
  UnexpectedCommand(crate::message::Command),

  #[fail(display = "unexpected data: {:?}", _0)]
  UnexpectedData(Vec<u8>),

  #[fail(display = "disconnected")]
  Disconnected,

  #[fail(display = "fail: {}", _0)]
  Fail(String),
}

impl AdbError {
  pub fn from_unexpected_command_u32(cmd: u32) -> Self {
    use crate::message::Command;
    use num_traits::FromPrimitive;
    if let Some(cmd) = Command::from_u32(cmd) {
      AdbError::UnexpectedCommand(cmd)
    } else {
      AdbError::UnknownCommand(cmd)
    }
  }
}

pub type AdbResult<T> = Result<T, AdbError>;

impl From<::std::io::Error> for AdbError {
  fn from(err: ::std::io::Error) -> AdbError {
    AdbError::Io(err)
  }
}
