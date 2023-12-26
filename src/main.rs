use crate::analysis::Analysis;
use crate::journalctl::{JournalPriority};
use clap::{arg, command};
use clap::{Parser};
mod journalctl;
mod analysis;

#[derive(Parser)]
#[command(author, version, about, long_about = None)]
struct Cli {
    /// The amount of boots to analyze
    #[arg(short, long, default_value_t = 25)]
    boots: usize,

    /// The minimum priority level of entries to analyze.
    #[arg(short, long, value_enum, default_value_t=JournalPriority::Error)]
    priority: JournalPriority,

    /// Get data from oldest boots
    #[arg(short, long, default_value_t = false)]
    old: bool,

    /// Reverse output showing newest first
    #[arg(short, long, default_value_t = false)]
    reverse: bool,

    /// Use ANSI escape codes to display color
    #[arg(short, long, default_value_t = true)]
    color: bool,
}

fn main() {
    let cli = Cli::parse();
    if cli.priority > JournalPriority::Error {
        println!("NOTICE: The chosen priority is lower than the default value.\nParsing lower priorities increases the amount of entries to be processed exponentially and with that the time to generate a result.")
    }
    let new_logs = journalctl::JournalBootIterator::new(Some(cli.priority), !cli.old).expect("Failed to get journal iterator");
    let mut analysis = Analysis::new();
    let mut boots = 0;
    for boot_entry in new_logs {
        match boot_entry {
            Ok(boot_entry) => {
                boots += 1;
                if boots > cli.boots {
                    break;
                }
                analysis.add_boot_entry(&boot_entry);
            }
            Err(boot_entry_error) => {
                let boot_str = match boot_entry_error.boot_id {
                    None => {"?".to_string()}
                    // u128 -> hex string
                    Some(id) => { format!("{:X}", id) }
                };
                println!("Failed to get results for boot {} because of:\n{}", boot_str, boot_entry_error.error.to_string());
            }
        }
    }
    analysis.print(175, cli.reverse, cli.color);
}
