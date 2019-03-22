use crate::result::AdbResult;
use std::io::prelude::*;

use super::{Command, Header};

#[derive(Debug)]
pub struct Connect {
  system_identity: String,
}

impl Connect {
  pub fn new(system_identity: &str) -> Self {
    Connect {
      system_identity: system_identity.to_string(),
    }
  }

  pub fn encode<W>(&self, w: &mut W) -> AdbResult<()>
  where
    W: Write,
  {
    Header::new(Command::A_CNXN)
      .arg0(crate::VERSION)
      .arg1(crate::MAX_DATA)
      .data(self.system_identity.as_bytes())
      .finalize()
      .encode(w)?;
    w.write_all(self.system_identity.as_bytes())?;
    Ok(())
  }
}
