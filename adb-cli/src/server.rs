use pretty_hex::*;
use std::io::prelude::*;
use std::net::{TcpListener, TcpStream};

pub fn run_fake() {
  let listener = TcpListener::bind("127.0.0.1:5037").unwrap();
  println!("fake server started.");
  for stream in listener.incoming() {
    handle_client(stream.unwrap());
  }
}

fn handle_client(mut stream: TcpStream) {
  println!("new connection.");
  let mut buf: [u8; 4096] = [0; 4096];
  loop {
    match stream.read(&mut buf) {
      Ok(n) => {
        if n == 0 {
          println!("done");
          break;
        }

        println!("{:?}", pretty_hex(&&buf[0..n]));
      }
      Err(err) => {
        println!("read error: {:?}", err);
        break;
      }
    }
  }
}

pub fn version() {
  let mut stream = TcpStream::connect("127.0.0.1:5037").unwrap();
  stream.write_all(b"000chost:version").unwrap();

  let mut buf = [0; 4096];
  loop {
    let n = stream.read(&mut buf).unwrap();
    println!("{:?}", pretty_hex(&&buf[0..n]));
    if n == 0 {
      break;
    }
  }
}

pub fn kill() {
  let mut stream = TcpStream::connect("127.0.0.1:5037").unwrap();
  stream.write_all(b"0009host:kill").unwrap();

  let mut buf = [0; 4096];
  loop {
    let n = stream.read(&mut buf).unwrap();
    println!("{}", pretty_hex(&&buf[0..n]));
    if n == 0 {
      break;
    }
  }
}
