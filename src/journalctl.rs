use std::io::{BufRead, BufReader, Error, Split};
use std::iter::Peekable;
use std::process::{ChildStdout, Command, Stdio};
use std::str::FromStr;
use serde_json::Value;

#[derive(Debug, Eq, Hash, PartialEq, Clone, Ord, PartialOrd, clap::ValueEnum)]
pub enum JournalPriority {
    Emerge = 0,
    Alert = 1,
    Critical = 2,
    Error = 3,
    Warning = 4,
    Notice = 5,
    Info = 6,
    Debug = 7,
}
impl JournalPriority {
    fn from_num(n: u8) -> Option<JournalPriority> {
        match n {
            0 => Some(JournalPriority::Emerge),
            1 => Some(JournalPriority::Alert),
            2 => Some(JournalPriority::Critical),
            3 => Some(JournalPriority::Error),
            4 => Some(JournalPriority::Warning),
            5 => Some(JournalPriority::Notice),
            6 => Some(JournalPriority::Info),
            7 => Some(JournalPriority::Debug),
            _ => None,
        }
    }
    pub fn ansi_color(&self) -> &str {
        match self {
            JournalPriority::Emerge => { "\x1b[35;1m" }
            JournalPriority::Alert => { "\x1b[35;1m" }
            JournalPriority::Critical => { "\x1b[31;1m" }
            JournalPriority::Error => { "\x1b[31m" }
            JournalPriority::Warning => { "\x1b[33m" }
            JournalPriority::Notice => { "\x1b[36;1m" }
            JournalPriority::Info => { "" }
            JournalPriority::Debug => { "\x1b[3m" }
        }
    }
}

const RECORD_SEPARATOR_CHAR: u8 = 30;

/// Wrapper for safe access
pub struct JournalEntry {
    pub value: Value,
}
pub type BootId = u128;
impl JournalEntry {
    fn from_bytes(bytes: &[u8]) -> Option<JournalEntry> {
        let parsed_json: Value = serde_json::from_slice(bytes).ok()?;
        let journal_entry = JournalEntry { value: parsed_json };
        return Some(journal_entry)
    }

    pub fn get_identifier(&self) -> Option<&str> {
        Some(self.value.get("SYSLOG_IDENTIFIER").or_else(|| self.value.get("_COMM"))?.as_str()?)
    }
    pub fn get_boot_id(&self) -> Option<BootId> {
        Some(u128::from_str_radix(self.value.get("_BOOT_ID")?.as_str()?, 16).ok()?)
    }
    pub fn get_priority(&self) -> Option<JournalPriority> {
        Some(JournalPriority::from_num(u8::from_str(self.value.get("PRIORITY")?.as_str()?).ok()?)?)
    }
    pub fn get_timestamp(&self) -> Option<u128> {
        Some(u128::from_str(self.value.get("__REALTIME_TIMESTAMP")?.as_str()?).ok()?)
    }
    pub fn get_message(&self) -> Option<&str> {
        Some(self.value.get("MESSAGE")?.as_str().unwrap_or(""))
    }
    #[allow(dead_code)]
    pub fn get_sequence_number_id(&self) -> Option<u128> {
        Some(u128::from_str_radix(self.value.get("__SEQNUM_ID")?.as_str()?, 16).ok()?)
    }
}

pub struct JournalBootIterator {
    reader: Peekable<Split<BufReader<ChildStdout>>>,
}

impl JournalBootIterator {
    pub fn new(minimum_priority: Option<JournalPriority>, newest_first: bool) -> Option<JournalBootIterator> {
        let mut cmd = Command::new("journalctl");
        cmd.stdout(Stdio::piped());
        cmd.args(["--output=json-seq", "--no-pager"]);
        if let Some(priority) = minimum_priority {
            cmd.arg(format!("-p 0..{}", priority as isize));
        }
        if newest_first {
            cmd.arg("-r");
        }

        let reader = cmd
            .spawn()
            .ok()
            .map(|child| child.stdout.map(|stdout| BufReader::new(stdout)))
            .flatten()?;

        let mut split = reader.split(RECORD_SEPARATOR_CHAR);
        split.next(); // remove first empty entry

        let journal_iterator = JournalBootIterator {
            reader: split.peekable(),
        };

        Some(journal_iterator)
    }
}

pub struct JournalBootEntry {
    pub entries: Vec<JournalEntry>,
    pub boot_id: BootId,
    pub start_timestamp: Option<u128>,
}
pub struct JournalBootEntryError {
    pub error: Error,
    pub boot_id: Option<BootId>,
}

impl Iterator for JournalBootIterator {
    type Item = Result<JournalBootEntry, JournalBootEntryError>;

    fn next(&mut self) -> Option<Self::Item> {
        let first_entry_bytes = match self.reader.next()? {
            Ok(bytes) => bytes,
            Err(e) => return Some(Err(JournalBootEntryError {
                error: e,
                boot_id: None,
            }))
        };
        let first_entry = JournalEntry::from_bytes(&first_entry_bytes)?;
        let start_timestamp = first_entry.get_timestamp();
        let first_boot_id = first_entry.get_boot_id().expect("Failed to get boot ID");
        let mut entries: Vec<JournalEntry> = Vec::new();
        entries.push(first_entry);

        while let Some(peeked_bytes) = self.reader.peek() {
            let entry_bytes = match peeked_bytes.as_ref() {
                Ok(bytes) => bytes,
                Err(e) => return Some(Err(JournalBootEntryError {
                    error: Error::from(e.kind()),
                    boot_id: Some(first_boot_id),
                }))
            };
            let entry = JournalEntry::from_bytes(entry_bytes)?;
            let boot_id_entry = entry.get_boot_id().expect("Failed to get boot ID");
            if boot_id_entry != first_boot_id {
                break;
            }
            // Discard entry from iterator after pushing it to the vector
            entries.push(entry);
            let _ = &self.reader.next()?;
        }

        return Some(Ok(JournalBootEntry {
            entries: entries,
            boot_id: first_boot_id,
            start_timestamp: start_timestamp,
        }));
    }
}
