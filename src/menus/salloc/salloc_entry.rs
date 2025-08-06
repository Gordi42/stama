use serde::{Deserialize, Serialize};

/// This module contains the SallocEntry struct, which is used to store the
/// paramters for the salloc command.
#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct SallocEntry {
    pub preset_name: String,
    pub account: String,
    pub partition: String,
    pub nodes: String,
    pub cpus_per_node: String,
    pub memory: String,
    pub time_limit: String,
    pub other_options: String,
}

// ====================================================================
//  CONSTRUCTOR
// ====================================================================

impl SallocEntry {
    /// Create a new SallocEntry with default values.
    pub fn new() -> Self {
        SallocEntry {
            preset_name: "new".to_string(), // "new" is the default preset name
            account: String::new(),
            partition: String::new(),
            nodes: String::new(),
            cpus_per_node: String::new(),
            memory: String::new(),
            time_limit: "01:00:00".to_string(),
            other_options: String::new(),
        }
    }
}

// ====================================================================
//  METHODS
// ====================================================================

impl SallocEntry {
    /// Start the salloc command with the parameters
    /// stored in this SallocEntry.
    pub fn start(&self) -> String {
        let mut cmd = "salloc".to_string();
        if !self.account.is_empty() {
            cmd.push_str(&format!(" --account={}", self.account));
        }
        if !self.partition.is_empty() {
            cmd.push_str(&format!(" --partition={}", self.partition));
        }
        if !self.nodes.is_empty() {
            cmd.push_str(&format!(" -N {}", self.nodes));
        }
        if !self.cpus_per_node.is_empty() {
            cmd.push_str(&format!(" --ntasks-per-node={}", self.cpus_per_node));
        }
        if !self.memory.is_empty() {
            cmd.push_str(&format!(" --mem={}", self.memory));
        }
        if !self.time_limit.is_empty() {
            cmd.push_str(&format!(" --time={}", self.time_limit));
        }
        if !self.other_options.is_empty() {
            cmd.push_str(&format!(" {}", self.other_options));
        }
        if !self.preset_name.is_empty() {
            cmd.push_str(&format!(" --job-name={}", self.preset_name));
        }
        cmd
    }
}
