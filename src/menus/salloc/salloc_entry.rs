use serde::{Deserialize, Serialize};


/// This module contains the SallocEntry struct, which is used to store the
/// paramters for the salloc command.
#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct SallocEntry {
    pub preset_name: String,
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
            partition: String::new(),
            nodes: String::new(),
            cpus_per_node: String::new(),
            memory: String::new(),
            time_limit: String::new(),
            other_options: String::new(),
        }
    }
}
