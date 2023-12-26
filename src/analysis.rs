use std::collections::HashMap;
use std::time::SystemTime;
use crate::journalctl::{BootId, JournalBootEntry, JournalEntry, JournalPriority};

#[derive(Debug)]
struct MessageEntry {
    _time: u128,
    message: String,
}

type Identifier = String;

#[derive(Debug)]
pub struct BootData {
    entries_by_priority: HashMap<JournalPriority, HashMap<Identifier, Vec<MessageEntry>>>,
    timestamp: u128,
}
impl BootData {
    fn new(timestamp: u128) -> BootData {
        BootData {
            entries_by_priority: Default::default(),
            timestamp: timestamp,
        }
    }
    fn add_entry(&mut self, entry: &JournalEntry) {
        let priority = entry.get_priority().expect("Failed to get priority");
        let identifier = entry.get_identifier().expect("Failed to get identifier").to_string();

        let priority_entry = self.entries_by_priority.entry(priority.clone()).or_insert_with(|| Default::default());
        let identifier_entry = priority_entry.entry(identifier.clone()).or_insert_with(|| Default::default());
        identifier_entry.push(MessageEntry {
            _time: entry.get_timestamp().expect("Failed to get timestamp"),
            message: entry.get_message().expect("Failed to get message").to_string(),
        });
    }
}

pub struct Analysis {
    pub by_boot: HashMap<BootId, BootData>,
}
impl Analysis {
    pub fn new() -> Analysis {
        Analysis {
            by_boot: HashMap::new(),
        }
    }
    /// Whether to ignore entry
    fn filter(entry: &JournalEntry) -> bool {
        let identifier = entry.get_identifier().expect("Failed to get journal entry identifier");
        // Failed logins aren't system issues
        let ignore_list = ["sudo"];
        return ignore_list.contains(&&*identifier);
    }
    pub(crate) fn add_boot_entry(&mut self, entry: &JournalBootEntry) -> Option<bool> {
        // Skips adding the boot entry if it already has been included
        if self.by_boot.contains_key(&entry.boot_id) {
            return Some(false);
        }

        let mut boot_data = BootData::new(entry.start_timestamp?);
        for journal_entry in &entry.entries {
            if !Analysis::filter(journal_entry) {
                boot_data.add_entry(journal_entry);
            }
        }
        self.by_boot.insert(entry.boot_id, boot_data);

        Some(true)
    }
    pub fn print(&self, message_max_length: usize, reverse_order: bool, color: bool) {
        let mut boot_entries = self.by_boot.iter().collect::<Vec<_>>();
        boot_entries.sort_by_key(|e| e.1.timestamp);
        if !reverse_order {
            boot_entries.reverse();
        }
        let current_epoch = SystemTime::now().duration_since(SystemTime::UNIX_EPOCH).expect("Failed to get system time").as_micros();
        for (boot_id, boot_data) in boot_entries {
            let microseconds_since = current_epoch - boot_data.timestamp;
            const MICROSECONDS_HOURS: u128 = 3600_000_000;
            let hours_since = microseconds_since / MICROSECONDS_HOURS;

            let boot_string;
            let hex_boot_id = format!("{:X}", boot_id);
            const HIGHLIGHTED_CHARS: usize = 4;
            if color {
                const ANSI_HIGHLIGHT_START: &str = "\x1b[1;4m";
                const ANSI_HIGHLIGHT_RESET: &str = "\x1b[0m";
                let mut str = String::with_capacity(ANSI_HIGHLIGHT_START.len() + hex_boot_id.len() + ANSI_HIGHLIGHT_RESET.len());
                str.push_str(ANSI_HIGHLIGHT_START);
                str.push_str(&hex_boot_id[..HIGHLIGHTED_CHARS]);
                str.push_str(ANSI_HIGHLIGHT_RESET);
                str.push_str(&hex_boot_id[HIGHLIGHTED_CHARS..]);
                boot_string = str;
            }
            else {
                boot_string = hex_boot_id;
            }

            let time_string: String = if color { format!("\x1b[3m({} hours ago)\x1b[0m", hours_since) } else { format!("({} hours ago)", hours_since) };

            println!("Boot {} {}", boot_string, time_string);
            let mut priority_entries = boot_data.entries_by_priority.iter().collect::<Vec<_>>();
            priority_entries.sort_by_key(|e| e.0);
            for (priority, map) in priority_entries {
                let (priority_color, ansi_reset) = if color { (priority.ansi_color(), "\x1b[0m") } else { ("", "") };
                let entry_priority_count = map.iter().map(|e| e.1.iter().count()).sum::<usize>();
                println!("├─ {}{:?}: {}{}", priority_color, priority, entry_priority_count, ansi_reset);
                let mut identifier_entries = map.iter().collect::<Vec<_>>();
                identifier_entries.sort_by_key(|e| e.0);
                for (identifier, msg_entries) in identifier_entries {
                    println!("│  ├─ {}: {}", identifier, msg_entries.iter().count());
                    let mut iter = msg_entries.iter().peekable();
                    let mut skipped = 0;
                    while let Some(entry) = iter.next() {
                        if iter.peek().map(|entry| entry.message == entry.message).unwrap_or(false) {
                            skipped += 1;
                        }
                        else {
                            let capped_message: &str;
                            let suffix: &str;
                            if entry.message.len() > message_max_length {
                                capped_message = &entry.message[..message_max_length];
                                suffix = "...";
                            }
                            else {
                                capped_message = &*entry.message;
                                suffix = "";
                            }
                            if skipped > 0 {
                                println!("│  │  │ {} x {:?}{}", skipped + 1, capped_message, suffix);
                            }
                            else {
                                println!("│  │  │ {:?}{}", capped_message, suffix);
                            }
                        }
                    }
                }
            }
        }
    }
}
