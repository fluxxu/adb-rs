use std::io::prelude::*;

use super::client::*;
use crate::result::*;

pub trait AdbShell {
  fn shell_exec(&mut self, cmd: &str) -> AdbResult<Vec<u8>>;
}

impl AdbShell for AdbConnection {
  fn shell_exec(&mut self, cmd: &str) -> AdbResult<Vec<u8>> {
    let stream = self.open_stream(&format!("shell:{}", cmd))?;
    let mut buf = vec![];

    loop {
      let packet = stream.recv()?;
      match packet.command {
        Command::A_WRTE => {
          buf.write_all(&packet.payload)?;
          stream.send_ok()?;
        }
        Command::A_CLSE => {
          break;
        }
        cmd => return Err(AdbError::UnexpectedCommand(cmd)),
      }
    }

    Ok(buf)
  }
}
