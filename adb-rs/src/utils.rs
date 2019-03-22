pub fn crc(buff: &[u8]) -> u32 {
  crc_seed(buff, 0)
}

pub fn crc_seed(buff: &[u8], seed: u32) -> u32 {
  let mut r = seed;
  for byte in buff {
    r = (r + *byte as u32) & 0xffffffff;
  }
  r
}
