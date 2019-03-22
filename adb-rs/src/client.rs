use bytes::buf::FromBuf;
use bytes::{Bytes, BytesMut};
use crossbeam_channel::{bounded, select, unbounded, Receiver, Sender};
use num_traits::FromPrimitive;
use std::collections::HashMap;
use std::io::prelude::*;
use std::net::{SocketAddr, TcpStream, ToSocketAddrs};
use std::sync::{Arc, RwLock};
use std::thread::{self, JoinHandle};

pub use crate::message::Command;
use crate::message::{Connect, Header};
use crate::result::*;

#[derive(Debug)]
pub struct AdbClient {
  system_identity: String,
}

impl AdbClient {
  pub fn new(system_identity: &str) -> Self {
    AdbClient {
      system_identity: system_identity.to_string(),
    }
  }

  pub fn connect<T>(self, addr: T) -> AdbResult<AdbConnection>
  where
    T: ToSocketAddrs,
  {
    let addrs: Vec<_> = addr.to_socket_addrs()?.collect();

    debug!("connecting to {:?}...", addrs);

    let mut stream = TcpStream::connect(&addrs as &[SocketAddr])?;

    debug!("connected. sending CNXN...");

    Connect::new(&self.system_identity).encode(&mut stream)?;

    let resp = Header::decode(&mut stream)?;
    let data = match Command::from_u32(resp.command) {
      Some(Command::A_CNXN) => resp.decode_data(&mut stream)?,
      Some(Command::A_AUTH) => {
        return Err(AdbError::AuthNotSupported);
      }
      Some(cmd) => {
        return Err(AdbError::UnexpectedCommand(cmd));
      }
      None => return Err(AdbError::UnknownCommand(resp.command)),
    };

    let device_id = String::from_utf8_lossy(&data);

    debug!(
      "handshake ok: device_id = {}, version = 0x{:x}, max_data = 0x{:x}",
      device_id, resp.arg0, resp.arg1
    );

    let streams = Arc::new(RwLock::new(HashMap::<u32, StreamContext>::new()));

    let (conn_reader_s, conn_reader_r) = bounded::<ConnectionPacket>(0);
    let (conn_writer_s, conn_writer_r) = bounded::<ConnectionPacket>(0);
    let (conn_error_s, conn_error_r) = unbounded();

    let reader_worker = thread::spawn({
      let mut stream = stream.try_clone()?;
      let error_s = conn_error_s.clone();
      move || loop {
        let res = Header::decode(&mut stream)
          .and_then(|header| {
            let mut payload = BytesMut::new();
            if header.data_length > 0 {
              payload.resize(header.data_length as usize, 0);
              stream
                .read_exact(&mut payload)
                .map(move |_| ConnectionPacket {
                  header,
                  payload: payload.freeze(),
                })
                .map_err(Into::into)
            } else {
              Ok(ConnectionPacket {
                header,
                payload: payload.freeze(),
              })
            }
          })
          .and_then(|packet| {
            conn_reader_s
              .send(packet)
              .map_err(|_| AdbError::Disconnected)
          });

        if let Err(err) = res {
          debug!("AdbConnection: reader_worker exited: {}", err);
          error_s.send(err).ok();
          break;
        }
      }
    });

    let writer_worker = thread::spawn({
      let mut stream = stream.try_clone()?;
      let streams = streams.clone();
      let error_s = conn_error_s.clone();
      move || {
        let mut closed_local_ids = vec![];
        let mut conn_dead = false;
        loop {
          let packet = conn_writer_r.recv();
          match packet {
            Ok(packet) => {
              let local_id = packet.header.arg0;
              let locked = streams.read().unwrap();
              if let Some(ctx) = locked.get(&local_id) {
                let write = packet
                  .header
                  .encode(&mut stream)
                  .and_then(|_| stream.write_all(&packet.payload).map_err(Into::into));
                match write {
                  Ok(_) => {
                    if let Err(_) = ctx.write_result_s.send(Ok(())) {
                      closed_local_ids.push(local_id);
                    }
                  }
                  Err(err) => {
                    if let Err(_) = ctx.write_result_s.send(Err(AdbError::Disconnected)) {
                      closed_local_ids.push(local_id);
                    }
                    conn_dead = true;
                    error_s.send(err).ok();
                  }
                }
              } else {
                warn!(
                  "write packet discarded: cmd = {}, local_id = {}",
                  packet.header.command, packet.header.arg0
                );
              }
            }
            Err(_) => {
              break;
            }
          }

          if !closed_local_ids.is_empty() {
            let mut locked = streams.write().unwrap();
            for id in &closed_local_ids {
              debug!("remove stream: local_id = {}", id);
              locked.remove(&id);
            }
            closed_local_ids.clear();
          }

          if conn_dead {
            break;
          }
        }
      }
    });

    let dispatch_worker = thread::spawn({
      let streams = streams.clone();
      move || {
        let mut closed_local_ids = vec![];
        loop {
          select! {
            recv(conn_reader_r) -> packet => {
              match packet {
                Ok(packet) => {
                  let local_id = packet.header.arg1;
                  let locked = streams.read().unwrap();
                  match locked.get(&packet.header.arg1) {
                    Some(ctx) => {
                      if Command::from_u32(packet.header.command).is_some() {
                        if let Err(_) = ctx.stream_reader_s.send(packet) {
                          closed_local_ids.push(local_id);
                        }
                      } else {
                        error!(
                          "read packet discarded: unknown_cmd = 0x{:x}, local_id = {}",
                          packet.header.command,
                          packet.header.arg1
                        );
                      }
                    },
                    None => {
                      warn!("read packet discarded: cmd = 0x{:x}, local_id = {}",
                        packet.header.command,
                        packet.header.arg1
                      );
                    },
                  }
                },
                Err(err) => {
                  error!("recv conn_reader_r: {}", err);
                  break
                },
              }
            },
            recv(conn_error_r) -> err => {
              match err {
                Ok(_) => {
                  break;
                },
                Err(recv_err) => {
                  error!("recv conn_error_r: {}", recv_err);
                  break
                },
              }
            },
          }

          if !closed_local_ids.is_empty() {
            let mut locked = streams.write().unwrap();
            for id in &closed_local_ids {
              debug!("remove stream: local_id = {}", id);
              locked.remove(&id);
            }
            closed_local_ids.clear();
          }
        }
        debug!("dispatch worker exited.");
      }
    });

    Ok(AdbConnection {
      system_identity: self.system_identity,
      device_system_identity: device_id.to_string(),
      device_version: resp.arg0,
      device_max_data: resp.arg1,
      tcp_stream: Some(stream),
      local_id_counter: 0,
      workers: [reader_worker, writer_worker, dispatch_worker],
      streams,
      conn_writer_s,
    })
  }
}

struct ConnectionPacket {
  header: Header,
  payload: Bytes,
}

#[derive(Debug)]
pub struct AdbStreamPacket {
  pub command: Command,
  pub payload: Bytes,
}

impl AdbStreamPacket {
  pub fn new_write<T: AsRef<[u8]>>(payload: T) -> Self {
    let bytes = payload.as_ref();
    AdbStreamPacket {
      command: Command::A_WRTE,
      payload: Bytes::from_buf(bytes),
    }
  }

  pub fn check_command(&self, cmd: Command) -> AdbResult<()> {
    if self.command != cmd {
      Err(AdbError::UnexpectedCommand(self.command))
    } else {
      Ok(())
    }
  }
}

#[derive(Debug, Clone)]
struct StreamContext {
  local_id: u32,
  remote_id: u32,
  stream_reader_s: Sender<ConnectionPacket>,
  write_result_s: Sender<AdbResult<()>>,
}

#[derive(Debug)]
pub struct AdbConnection {
  system_identity: String,
  device_system_identity: String,
  device_version: u32,
  device_max_data: u32,
  tcp_stream: Option<TcpStream>,
  local_id_counter: u32,
  workers: [JoinHandle<()>; 3],
  streams: Arc<RwLock<HashMap<u32, StreamContext>>>,
  conn_writer_s: Sender<ConnectionPacket>,
}

impl AdbConnection {
  pub fn max_data_len(&self) -> usize {
    self.device_max_data as usize
  }

  pub fn open_stream(&mut self, destination: &str) -> AdbResult<AdbStream> {
    use bytes::BufMut;

    self.local_id_counter = self.local_id_counter + 1;
    let local_id = self.local_id_counter;
    debug!(
      "opening stream: local_id = {}, destination = {}...",
      local_id, destination
    );

    let (write_result_s, write_result_r) = bounded::<AdbResult<()>>(1);
    let (stream_reader_s, stream_reader_r) = bounded::<ConnectionPacket>(1);

    let ctx = StreamContext {
      local_id,
      remote_id: 0,
      stream_reader_s,
      write_result_s,
    };

    self.streams.write().unwrap().insert(local_id, ctx);
    debug!("register stream: local_id = {}", local_id);

    let mut dst_bytes = BytesMut::with_capacity(destination.as_bytes().len() + 1);
    dst_bytes.extend(destination.as_bytes());
    dst_bytes.put_u8(0);
    let dst_bytes = dst_bytes.freeze();

    let open_packet = ConnectionPacket {
      header: Header::new(Command::A_OPEN)
        .arg0(local_id)
        .data(&dst_bytes)
        .finalize(),
      payload: dst_bytes,
    };

    self
      .conn_writer_s
      .send(open_packet)
      .map_err(|_| AdbError::Disconnected)?;

    let open_packet = stream_reader_r.recv().map_err(|_| AdbError::Disconnected)?;
    if open_packet.header.command != Command::A_OKAY as u32 {
      if let Some(cmd) = Command::from_u32(open_packet.header.command) {
        return Err(AdbError::UnexpectedCommand(cmd));
      } else {
        return Err(AdbError::UnknownCommand(open_packet.header.command));
      }
    }

    let local_id = open_packet.header.arg1;
    let remote_id = open_packet.header.arg0;
    debug!("stream opened: {} -> {}", local_id, remote_id);

    Ok(AdbStream {
      local_id,
      remote_id,
      stream_reader: stream_reader_r,
      writer: self.conn_writer_s.clone(),
      write_result_r,
    })
  }
}

#[derive(Debug)]
pub struct AdbStream {
  local_id: u32,
  remote_id: u32,
  stream_reader: Receiver<ConnectionPacket>,
  writer: Sender<ConnectionPacket>,
  write_result_r: Receiver<AdbResult<()>>,
}

impl AdbStream {
  pub fn send(&self, packet: AdbStreamPacket) -> AdbResult<()> {
    self
      .writer
      .send(ConnectionPacket {
        header: Header::new(packet.command)
          .arg0(self.local_id)
          .arg1(self.remote_id)
          .data(&packet.payload)
          .finalize(),
        payload: packet.payload,
      })
      .map_err(|_| AdbError::Disconnected)
      .and_then(|_| {
        self
          .write_result_r
          .recv()
          .map_err(|_| AdbError::Disconnected)
          .and_then(|res| res.map(|_| ()))
      })
  }

  pub fn recv(&self) -> AdbResult<AdbStreamPacket> {
    let packet = self
      .stream_reader
      .recv()
      .map_err(|_| AdbError::Disconnected)?;

    Ok(AdbStreamPacket {
      command: Command::from_u32(packet.header.command)
        .ok_or_else(|| AdbError::UnknownCommand(packet.header.command))?,
      payload: packet.payload,
    })
  }

  pub fn try_recv(&self) -> AdbResult<Option<AdbStreamPacket>> {
    use crossbeam_channel::TryRecvError;
    match self.stream_reader.try_recv() {
      Ok(packet) => Ok(Some(AdbStreamPacket {
        command: Command::from_u32(packet.header.command)
          .ok_or_else(|| AdbError::UnknownCommand(packet.header.command))?,
        payload: packet.payload,
      })),
      Err(TryRecvError::Empty) => Ok(None),
      Err(TryRecvError::Disconnected) => Err(AdbError::Disconnected),
    }
  }

  pub fn send_ok(&self) -> AdbResult<()> {
    self.send(AdbStreamPacket {
      command: Command::A_OKAY,
      payload: Bytes::new(),
    })
  }

  pub fn recv_command(&self, cmd: Command) -> AdbResult<AdbStreamPacket> {
    let packet = self.recv()?;
    if packet.command != cmd {
      return Err(AdbError::UnexpectedCommand(packet.command));
    }
    Ok(packet)
  }

  pub fn send_close(&self) -> AdbResult<()> {
    self.send(AdbStreamPacket {
      command: Command::A_CLSE,
      payload: Bytes::new(),
    })
  }
}
