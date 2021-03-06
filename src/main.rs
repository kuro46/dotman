#[macro_use]
extern crate anyhow;
#[macro_use]
extern crate log;
#[macro_use]
extern crate clap;

mod app;

use app::App;
use clap::{App as ClapApp, AppSettings, Arg, SubCommand};
use std::vec::Vec;

fn main() {
    pretty_env_logger::init();
    let m = ClapApp::new("dotman")
        .author(crate_authors!())
        .version(crate_version!())
        .subcommand(SubCommand::with_name("mappings").about("Prints list of mappings"))
        .subcommand(
            SubCommand::with_name("status").about("Executes 'git status' in dotfiles folder"),
        )
        .subcommand(SubCommand::with_name("restore").about("Not implemented now"))
        .subcommand(
            SubCommand::with_name("git")
                .about("Executes any subcommands of git in dotfiles folder")
                .setting(AppSettings::TrailingVarArg)
                .arg(Arg::with_name("args").required(false).multiple(true)),
        )
        .subcommand(SubCommand::with_name("unlink")
            .about("Unlink mapped file")
            .arg(Arg::with_name("source")))
        .subcommand(
            SubCommand::with_name("link")
                .about("Link specified file")
                .arg(Arg::with_name("source"))
                .arg(Arg::with_name("dest")),
        )
        .get_matches();
    let mut app = App::new().unwrap();
    match m.subcommand_name().unwrap_or_else(|| "status") {
        "mappings" => app.mappings(),
        "status" => app.status(),
        "restore" => app.restore(),
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
        unknown => panic!("'{}' IS UNKNOWN SUBCOMMAND!", unknown),
    }
}
