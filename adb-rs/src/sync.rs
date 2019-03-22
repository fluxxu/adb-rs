use bytes::*;
use std::io::prelude::*;

use crate::client::{AdbStream, AdbStreamPacket, Command};
use crate::result::*;

#[allow(unused)]
#[derive(Debug)]
pub enum SyncCommand {
  LIST = 0x5453494c,
  RECV = 0x56434552,
  SEND = 0x444e4553,
  STAT = 0x54415453,
  DATA = 0x41544144,
  DENT = 0x544e4544,
  OKAY = 0x59414b4f,
  DONE = 0x454e4f44,
  QUIT = 0x54495551,
  FAIL = 0x4c494146,
}

impl SyncCommand {
  pub fn new_send(name: &str, mode: u32) -> SyncPacket {
    let data_str = format!("{},{}", name, mode);
    let data = data_str.as_bytes();
    let header = SyncHeader {
      id: SyncCommand::SEND as u32,
      length: data.len() as u32,
    };
    let mut bytes = BytesMut::with_capacity(8 + data.len());
    bytes.put_slice(&header.bytes());
    bytes.put_slice(data);
    SyncPacket { header, bytes }
  }

  pub fn new_stat(name: &str) -> SyncPacket {
    let data_str = format!("{}\0", name);
    let data = data_str.as_bytes();
    let header = SyncHeader {
      id: SyncCommand::STAT as u32,
      length: data.len() as u32,
    };
    let mut bytes = BytesMut::with_capacity(8 + data.len());
    bytes.put_slice(&header.bytes());
    bytes.put_slice(data);
    SyncPacket { header, bytes }
  }

  pub fn new_data(max_size: usize) -> SyncPacket {
    let header = SyncHeader {
      id: SyncCommand::DATA as u32,
      length: max_size as u32 - 8,
    };
    let mut bytes = BytesMut::with_capacity(max_size);
    bytes.put_slice(&header.bytes());
    SyncPacket { header, bytes }
  }

  pub fn new_done(mtime: u32) -> [u8; 8] {
    let header = SyncHeader {
      id: SyncCommand::DONE as u32,
      length: mtime,
    };
    header.bytes()
  }

  pub fn new_quit() -> [u8; 8] {
    let header = SyncHeader {
      id: SyncCommand::QUIT as u32,
      length: 0,
    };
    header.bytes()
  }
}

#[derive(Debug, Default)]
pub struct SyncHeader {
  pub id: u32,
  pub length: u32,
}

#[derive(Debug, Default)]
pub struct SyncPacket {
  pub header: SyncHeader,
  pub bytes: BytesMut,
}

impl SyncPacket {
  pub fn extend<T: AsRef<[u8]>>(&mut self, data: T) {
    self.bytes.extend_from_slice(data.as_ref());
  }

  pub fn len(&self) -> usize {
    self.bytes.len()
  }

  // pub fn payload_len(&self) -> usize {
  //   self.bytes.len() - 8
  // }

  // pub fn payload_slice(&self) -> &[u8] {
  //   &self.bytes[8..]
  // }

  pub fn read_payload_from<R: Read>(&mut self, r: &mut R) -> AdbResult<usize> {
    self.bytes.resize(self.bytes.capacity(), 0);
    let n = r.read(&mut self.bytes[8..])?;
    self.bytes.truncate(8 + n);
    self.header.length = n as u32;
    (&mut self.bytes[0..8]).write_all(&self.header.bytes())?;
    Ok(n)
  }
}

impl AsRef<[u8]> for SyncPacket {
  fn as_ref(&self) -> &[u8] {
    self.bytes.as_ref()
  }
}

impl SyncHeader {
  pub fn from_bytes(bytes: &[u8]) -> Option<Self> {
    if bytes.len() != 8 {
      return None;
    }
    let id = LittleEndian::read_u32(&bytes);
    let length = LittleEndian::read_u32(&bytes[4..]);
    Some(SyncHeader { id, length })
  }

  fn bytes(&self) -> [u8; 8] {
    let mut header_bytes = [0; 8];
    LittleEndian::write_u32(&mut header_bytes, self.id);
    LittleEndian::write_u32(&mut header_bytes[4..], self.length);
    header_bytes
  }
}

pub trait SyncStreamExt {
  fn sync_recv(&self) -> AdbResult<AdbStreamPacket>;
  fn sync_recv_command(&self, cmd: Command) -> AdbResult<AdbStreamPacket> {
    let packet = self.sync_recv()?;

    packet.check_command(cmd)?;

    Ok(packet)
  }

  fn sync_recv_ok(&self) -> AdbResult<()> {
    self.sync_recv_command(Command::A_OKAY).map(|_| ())
  }
}
impl SyncStreamExt for AdbStream {
  fn sync_recv(&self) -> AdbResult<AdbStreamPacket> {
    let packet = self.recv()?;

    if packet.command == Command::A_WRTE && packet.payload.len() == 8 {
      let sync_header = SyncHeader::from_bytes(&packet.payload);
      if let Some(header) = sync_header {
        if header.id == SyncCommand::FAIL as u32 {
          return fail(self, header.length as usize);
        }
      }
    }

    Ok(packet)
  }
}

fn fail(stream: &AdbStream, len: usize) -> AdbResult<AdbStreamPacket> {
  loop {
    let packet = stream.recv().map_err(|_| AdbError::Disconnected)?;
    match packet.command {
      Command::A_OKAY => {
        stream.send_ok()?;
      }
      Command::A_WRTE => {
        return Err(AdbError::Fail(
          String::from_utf8_lossy(&packet.payload[0..len]).to_string(),
        ));
      }
      cmd => return Err(AdbError::UnexpectedCommand(cmd)),
    }
  }
}
