// #[macro_use]
extern crate log;

use simplelog::*;

use clap::load_yaml;
use clap::App;

mod push;
mod server;
mod shell;

fn main() {
  CombinedLogger::init(vec![
    TermLogger::new(LevelFilter::Debug, Config::default()).unwrap()
  ])
  .unwrap();

  let yaml = load_yaml!("cli.yml");
  let matches = App::from_yaml(yaml).get_matches();

  if let Some(_) = matches.subcommand_matches("server-fake") {
    return server::run_fake();
  }

  if let Some(_) = matches.subcommand_matches("server-version") {
    return server::version();
  }

  if let Some(_) = matches.subcommand_matches("server-kill") {
    return server::kill();
  }

  if let Some(m) = matches.subcommand_matches("shell") {
    return shell::run(m.value_of("CMD").unwrap());
  }

  if let Some(m) = matches.subcommand_matches("push") {
    return push::run(m.value_of("SRC").unwrap(), m.value_of("DST").unwrap());
  }
}
