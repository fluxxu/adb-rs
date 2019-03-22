use adb_rs::shell::AdbShell;
use adb_rs::AdbClient;

pub fn run(cmd: &str) {
  use std::io::{stdout, Write};

  let mut conn = AdbClient::new("host::").connect("127.0.0.1:5555").unwrap();

  stdout().write_all(&conn.shell_exec(cmd).unwrap()).unwrap();
}
