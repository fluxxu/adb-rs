use crate::result::*;
use bytes::{ByteOrder, LittleEndian};
use num_traits::FromPrimitive;
use std::io::prelude::*;

use crate::utils;

mod connect;

pub use self::connect::Connect;

#[allow(non_camel_case_types)]
#[repr(u32)]
#[derive(Debug, Copy, Clone, FromPrimitive, PartialEq)]
pub enum Command {
  A_SYNC = 0x434e5953,
  A_CNXN = 0x4e584e43,
  A_AUTH = 0x48545541,
  A_OPEN = 0x4e45504f,
  A_OKAY = 0x59414b4f,
  A_CLSE = 0x45534c43,
  A_WRTE = 0x45545257,
}

#[derive(Debug, Default)]
pub struct Header {
  pub command: u32,
  pub arg0: u32,
  pub arg1: u32,
  pub data_length: u32,
  pub data_crc32: u32,
  pub magic: u32,
}

impl Header {
  pub fn new(command: Command) -> HeaderBuilder {
    HeaderBuilder {
      inner: Header {
        command: command as u32,
        magic: command as u32 ^ 0xffffffff,
        ..Default::default()
      },
    }
  }

  pub fn get_command(&self) -> Option<Command> {
    Command::from_u32(self.command)
  }

  pub fn encode<W>(&self, w: &mut W) -> AdbResult<()>
  where
    W: Write,
  {
    let mut buf = [0; 24];

    LittleEndian::write_u32(&mut buf, self.command);
    LittleEndian::write_u32(&mut buf[4..], self.arg0);
    LittleEndian::write_u32(&mut buf[8..], self.arg1);
    LittleEndian::write_u32(&mut buf[12..], self.data_length);
    LittleEndian::write_u32(&mut buf[16..], self.data_crc32);
    LittleEndian::write_u32(&mut buf[20..], self.command ^ 0xffffffff);

    w.write_all(&mut buf)?;
    Ok(())
  }

  pub fn decode<R>(r: &mut R) -> AdbResult<Self>
  where
    R: Read,
  {
    let mut buf = [0; 24];
    r.read_exact(&mut buf)?;

    Ok(Header {
      command: LittleEndian::read_u32(&buf),
      arg0: LittleEndian::read_u32(&buf[4..]),
      arg1: LittleEndian::read_u32(&buf[8..]),
      data_length: LittleEndian::read_u32(&buf[12..]),
      data_crc32: LittleEndian::read_u32(&buf[16..]),
      magic: LittleEndian::read_u32(&buf[20..]),
    })
  }

  pub fn decode_data<R>(&self, r: &mut R) -> AdbResult<Vec<u8>>
  where
    R: Read,
  {
    let mut buf = Vec::with_capacity(self.data_length as usize);
    buf.resize(self.data_length as usize, 0);
    r.read_exact(&mut buf)?;
    if utils::crc(&buf) != self.data_crc32 {
      return Err(AdbError::Crc);
    }
    Ok(buf)
  }
}

#[derive(Debug, Default)]
pub struct HeaderBuilder {
  inner: Header,
}

impl<'a> From<&'a Header> for HeaderBuilder {
  fn from(header: &'a Header) -> HeaderBuilder {
    HeaderBuilder {
      inner: Header {
        command: header.command,
        arg0: header.arg0,
        arg1: header.arg1,
        data_length: header.data_length,
        data_crc32: header.data_crc32,
        magic: 0,
      },
    }
  }
}

impl HeaderBuilder {
  pub fn arg0<T: Into<u32>>(self, v: T) -> Self {
    HeaderBuilder {
      inner: Header {
        arg0: v.into(),
        ..self.inner
      },
    }
  }

  pub fn arg1<T: Into<u32>>(self, v: T) -> Self {
    HeaderBuilder {
      inner: Header {
        arg1: v.into(),
        ..self.inner
      },
    }
  }

  pub fn data<T: AsRef<[u8]>>(self, v: T) -> Self {
    let slice = v.as_ref();
    HeaderBuilder {
      inner: Header {
        data_length: slice.len() as u32,
        data_crc32: utils::crc(slice),
        ..self.inner
      },
    }
  }

  pub fn finalize(self) -> Header {
    self.inner
  }
}
