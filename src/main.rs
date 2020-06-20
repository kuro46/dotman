#[macro_use]
extern crate anyhow;
#[macro_use]
extern crate log;
#[macro_use]
extern crate clap;

mod app;

use app::App;
use std::vec::Vec;
use clap::{App as ClapApp, AppSettings, Arg, SubCommand};

fn main() {
    pretty_env_logger::init();
    let m = ClapApp::new("dotman")
        .author(crate_authors!())
        .version(crate_version!())
        .subcommand(SubCommand::with_name("status"))
        .subcommand(SubCommand::with_name("restore"))
        .subcommand(
            SubCommand::with_name("git")
                .setting(AppSettings::TrailingVarArg)
                .arg(Arg::with_name("args").required(false).multiple(true)),
        )
        .subcommand(SubCommand::with_name("unlink").arg(Arg::with_name("source")))
        .subcommand(
            SubCommand::with_name("link")
                .arg(Arg::with_name("source"))
                .arg(Arg::with_name("dest")),
        )
        .get_matches();
    let sub_name = match m.subcommand_name() {
        Some(sub_name) => sub_name,
        None => return,
    };
    let mut app = App::new().unwrap();
    match sub_name {
        "status" => {
            app.status();
        }
        "restore" => {
            app.restore();
        }
        "git" => {
            let sub_m = m.subcommand().1.unwrap();
            app.git(&sub_m.values_of_lossy("args").unwrap_or_else(Vec::new));
        }
        "unlink" => {
            let sub_m = m.subcommand().1.unwrap();
            app.unlink(sub_m.value_of("source").unwrap());
        }
        "link" => {
            let sub_m = m.subcommand().1.unwrap();
            app.link(
                sub_m.value_of("source").unwrap(),
                sub_m.value_of("dest").unwrap(),
            );
        }
        _ => {
            unimplemented!();
        }
    }
}
