use adb_rs::push::AdbPush;
use adb_rs::AdbClient;

pub fn run(src: &str, remote_path: &str) {
  let mut conn = AdbClient::new("host::").connect("127.0.0.1:5555").unwrap();

  conn.push(src, remote_path).unwrap();
}
