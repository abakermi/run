use chumsky::Parser as _;
use runner::Command;
pub use std::format as fmt;
use std::{collections::HashMap, ops::Deref};

mod parser;
mod runner;

fn main() -> std::io::Result<()> {
    let runfile = get_file();
    // let runfile = parser::parse(runfile.deref()).expect("Could not parse runfile");
    let runfile = match parser::runfile().parse(&runfile).into_result() {
        Ok(r) => r,
        Err(errors) => {
            for e in errors {
                eprintln!("{e:?}");
            }
            std::process::exit(1);
        }
    };

    let args = std::env::args().skip(1).collect::<Vec<_>>();
    match args.first() {
        Some(a) if a == "-c" || a == "--commands" => {
            println!("Available commands:");
            let mut commands = runfile.commands.values().collect::<Vec<_>>();
            commands.sort_by(|a, b| {
                if a.name == "default" {
                    std::cmp::Ordering::Less
                } else {
                    a.name.cmp(b.name)
                }
            });
            let max = commands
                .iter()
                .map(|c| c.name.len())
                .max()
                .unwrap_or_default();
            for cmd in commands {
                let doc = cmd.doc();
                let mut lines = doc.lines();
                println!(
                    "    {:max$}   {}",
                    cmd.name,
                    lines.next().unwrap(),
                    max = max
                );
                for l in lines {
                    println!("    {:max$}   {}", " ", l);
                }
            }
            return Ok(());
        }
        _ => {}
    }
    if args.first().is_some_and(|a| a == "-h" || a == "--help") {
        println!("Runs a runfile in the current directory");
        println!("Possible runfile names: [runfile, run, Runfile, Run]\n");
        println!("Usage: run [COMMAND] [ARGS...]\n");
        println!("Options:");
        println!("  -h, --help\t\tPrints help information");
        println!("  -c, --commands\tPrints available commands in the runfile");
        return Ok(());
    }

    match args.first().and_then(|c| runfile.commands.get(c.as_str())) {
        Some(cmd) => cmd.run(args.first().unwrap(), args.get(1..).unwrap_or_default())?,
        None => {
            let cmd = runfile.commands.get("default").byefmt(|| {
                fmt!(
                    "Could not find default command\nAvailable commands: {:?}",
                    runfile.commands.keys()
                )
            });
            cmd.run("default", args)?;
        }
    }

    Ok(())
}

fn get_file() -> String {
    let files = [
        "runfile",
        "run",
        "Runfile",
        "Run",
        "runfile.run",
        "run.run",
        "Runfile.run",
        "Run.run",
    ];
    for file in files {
        if let Ok(file) = std::fs::read_to_string(file) {
            return file;
        }
    }
    eprintln!("Could not find runfile");
    eprintln!("Possible file names: {files:?}");
    eprintln!("See `run --help` for more information");
    std::process::exit(1);
}

#[derive(Debug, PartialEq, Eq, Clone, Default)]
pub struct Runfile<'i> {
    commands: HashMap<&'i str, Command<'i>>,
    subcommands: HashMap<&'i str, Runfile<'i>>,
    includes: HashMap<&'i str, Runfile<'i>>,
}

trait Goodbye<T>
where
    Self: Sized,
{
    fn bye(self, msg: impl AsRef<str>) -> T {
        if let Some(t) = self.check() {
            return t;
        }
        eprintln!("{}", msg.as_ref());
        std::process::exit(1)
    }

    fn byefmt<S: AsRef<str>>(self, msg: impl Fn() -> S) -> T {
        if let Some(t) = self.check() {
            return t;
        }
        eprintln!("{}", msg().as_ref());
        std::process::exit(1)
    }

    fn check(self) -> Option<T>;
}

impl<T> Goodbye<T> for Option<T> {
    fn check(self) -> Option<T> {
        self
    }
}

impl<T, E> Goodbye<T> for Result<T, E> {
    fn check(self) -> Option<T> {
        self.ok()
    }
}

impl Goodbye<bool> for bool {
    fn check(self) -> Option<bool> {
        if self {
            Some(self)
        } else {
            None
        }
    }
}
