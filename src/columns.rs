//! The configurable columns of the job table.
//!
//! [`JobColumn`] enumerates every column the job overview can display.
//! Each column knows its config name (used in `config.toml` and in the
//! settings menu), its table header, its minimum display width and how
//! to extract its cell text from a [`Job`]. The columns to display come
//! from `UserOptions::job_columns`; the default set matches the
//! historical fixed table (ID, Name, Status, Time, Partition, Nodes).
//!
//! All underlying job fields are always fetched from squeue (see
//! `scheduler::squeue_format_arg`); the configuration only controls
//! which of them are *displayed*.

use serde::{Deserialize, Serialize};

use crate::job::Job;

/// A column of the job table. Also used as the sort category of the
/// job list: every column can be sorted by clicking its header.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum JobColumn {
    Id,
    Name,
    Status,
    Time,
    Partition,
    Nodes,
    Priority,
    Reason,
    Account,
    Qos,
    Cpus,
    NodeList,
}

/// Every column, in the order used for documentation and error hints.
pub const ALL_COLUMNS: [JobColumn; 12] = [
    JobColumn::Id,
    JobColumn::Name,
    JobColumn::Status,
    JobColumn::Time,
    JobColumn::Partition,
    JobColumn::Nodes,
    JobColumn::Priority,
    JobColumn::Reason,
    JobColumn::Account,
    JobColumn::Qos,
    JobColumn::Cpus,
    JobColumn::NodeList,
];

impl JobColumn {
    /// The default column set: the historical fixed job table.
    pub fn defaults() -> Vec<JobColumn> {
        vec![
            JobColumn::Id,
            JobColumn::Name,
            JobColumn::Status,
            JobColumn::Time,
            JobColumn::Partition,
            JobColumn::Nodes,
        ]
    }

    /// The name of the column in the config file / settings menu.
    pub fn config_name(&self) -> &'static str {
        match self {
            JobColumn::Id => "id",
            JobColumn::Name => "name",
            JobColumn::Status => "status",
            JobColumn::Time => "time",
            JobColumn::Partition => "partition",
            JobColumn::Nodes => "nodes",
            JobColumn::Priority => "priority",
            JobColumn::Reason => "reason",
            JobColumn::Account => "account",
            JobColumn::Qos => "qos",
            JobColumn::Cpus => "cpus",
            JobColumn::NodeList => "nodelist",
        }
    }

    /// The header text shown in the job table.
    pub fn header(&self) -> &'static str {
        match self {
            JobColumn::Id => "ID",
            JobColumn::Name => "Name",
            JobColumn::Status => "Status",
            JobColumn::Time => "Time",
            JobColumn::Partition => "Partition",
            JobColumn::Nodes => "Nodes",
            JobColumn::Priority => "Priority",
            JobColumn::Reason => "Reason",
            JobColumn::Account => "Account",
            JobColumn::Qos => "QOS",
            JobColumn::Cpus => "CPUs",
            JobColumn::NodeList => "Nodelist",
        }
    }

    /// The minimum display width of the column (the historical values
    /// for the six default columns, header width + sort arrow for the
    /// rest).
    pub fn min_width(&self) -> u16 {
        match self {
            JobColumn::Id => 8,
            JobColumn::Name => 10,
            JobColumn::Status => 8,
            JobColumn::Time => 6,
            JobColumn::Partition => 11,
            JobColumn::Nodes => 7,
            JobColumn::Priority => 10,
            JobColumn::Reason => 10,
            JobColumn::Account => 9,
            JobColumn::Qos => 5,
            JobColumn::Cpus => 6,
            JobColumn::NodeList => 10,
        }
    }

    /// The cell text of this column for the given job.
    pub fn cell(&self, job: &Job) -> String {
        match self {
            JobColumn::Id => job.id.clone(),
            JobColumn::Name => job.name.clone(),
            JobColumn::Status => job.status.to_string(),
            JobColumn::Time => format_time(job),
            JobColumn::Partition => job.partition.clone(),
            JobColumn::Nodes => job.nodes.to_string(),
            JobColumn::Priority => job.priority.to_string(),
            JobColumn::Reason => job.reason.clone().unwrap_or_default(),
            JobColumn::Account => job.account.clone(),
            JobColumn::Qos => job.qos.clone(),
            JobColumn::Cpus => job.cpus.to_string(),
            JobColumn::NodeList => job.nodelist.clone(),
        }
    }

    /// Parses a column from its config name (case-insensitive,
    /// surrounding whitespace is ignored).
    pub fn from_name(name: &str) -> Option<JobColumn> {
        let name = name.trim().to_ascii_lowercase();
        ALL_COLUMNS
            .into_iter()
            .find(|column| column.config_name() == name)
    }

    /// Returns the column after `self` in the given column list,
    /// wrapping around at the end. If `self` is not in the list (e.g.
    /// the sort column was removed from the configuration), the first
    /// column of the list is returned.
    pub fn next_in(&self, columns: &[JobColumn]) -> JobColumn {
        match columns.iter().position(|column| column == self) {
            Some(index) => columns[(index + 1) % columns.len()],
            None => *columns.first().unwrap_or(&JobColumn::Id),
        }
    }
}

/// Parses a comma-separated column list, e.g.
/// "id, name, status, time, partition, priority". Empty items are
/// skipped. Returns `None` if any name is unknown or no column remains,
/// so the caller can keep the previous value.
pub fn parse_columns(text: &str) -> Option<Vec<JobColumn>> {
    let mut columns = Vec::new();
    for item in text.split(',') {
        if item.trim().is_empty() {
            continue;
        }
        columns.push(JobColumn::from_name(item)?);
    }
    if columns.is_empty() {
        return None;
    }
    Some(columns)
}

/// Formats a column list as a comma-separated string (the inverse of
/// [`parse_columns`]).
pub fn columns_to_string(columns: &[JobColumn]) -> String {
    columns
        .iter()
        .map(|column| column.config_name())
        .collect::<Vec<_>>()
        .join(", ")
}

/// Formats the time string of a job for display: the "0-" day prefix
/// is dropped for jobs that run less than a day.
pub fn format_time(job: &Job) -> String {
    let time_str = job.time.clone();

    let parts: Vec<&str> = time_str.split('-').collect();
    match parts.len() {
        1 => parts[0].to_string(),
        2 => {
            let days = parts[0].parse::<i32>().unwrap_or(0);
            if days > 0 {
                time_str
            } else {
                parts[1].to_string()
            }
        }
        _ => "".to_string(),
    }
}

// ====================================================================
//  TESTS
// ====================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_columns_match_the_historical_table() {
        // regression guard: without configuration the job table must
        // keep its historical six columns in the same order
        let defaults = JobColumn::defaults();
        assert_eq!(
            defaults,
            vec![
                JobColumn::Id,
                JobColumn::Name,
                JobColumn::Status,
                JobColumn::Time,
                JobColumn::Partition,
                JobColumn::Nodes,
            ]
        );
        // ... and Tab must cycle through them in the same order
        let mut category = JobColumn::Id;
        let mut cycle = Vec::new();
        for _ in 0..6 {
            category = category.next_in(&defaults);
            cycle.push(category);
        }
        assert_eq!(
            cycle,
            vec![
                JobColumn::Name,
                JobColumn::Status,
                JobColumn::Time,
                JobColumn::Partition,
                JobColumn::Nodes,
                JobColumn::Id,
            ]
        );
    }

    #[test]
    fn test_next_in_falls_back_to_first_column() {
        let columns = vec![JobColumn::Id, JobColumn::Priority];
        // a sort category that is not displayed jumps to the first column
        assert_eq!(JobColumn::Name.next_in(&columns), JobColumn::Id);
        // an empty column list falls back to Id instead of panicking
        assert_eq!(JobColumn::Name.next_in(&[]), JobColumn::Id);
    }

    #[test]
    fn test_parse_columns() {
        // simple list
        assert_eq!(
            parse_columns("id,priority"),
            Some(vec![JobColumn::Id, JobColumn::Priority])
        );
        // whitespace and case are tolerated, empty items are skipped
        assert_eq!(
            parse_columns(" ID , Priority ,, NodeList "),
            Some(vec![
                JobColumn::Id,
                JobColumn::Priority,
                JobColumn::NodeList
            ])
        );
        // unknown names and empty lists are rejected
        assert_eq!(parse_columns("id,bogus"), None);
        assert_eq!(parse_columns(""), None);
        assert_eq!(parse_columns(" , "), None);
    }

    #[test]
    fn test_columns_round_trip_through_string() {
        for column in ALL_COLUMNS {
            assert_eq!(JobColumn::from_name(column.config_name()), Some(column));
        }
        let columns = ALL_COLUMNS.to_vec();
        assert_eq!(parse_columns(&columns_to_string(&columns)), Some(columns));
    }

    #[test]
    fn test_cell_extraction() {
        let mut job = Job::new_default();
        job.priority = 4294901760;
        job.account = "physics".to_string();
        job.qos = "normal".to_string();
        job.cpus = 32;
        job.nodelist = "l[42314-42316]".to_string();
        job.reason = Some("Priority".to_string());

        assert_eq!(JobColumn::Id.cell(&job), "123456");
        assert_eq!(JobColumn::Priority.cell(&job), "4294901760");
        assert_eq!(JobColumn::Account.cell(&job), "physics");
        assert_eq!(JobColumn::Qos.cell(&job), "normal");
        assert_eq!(JobColumn::Cpus.cell(&job), "32");
        assert_eq!(JobColumn::NodeList.cell(&job), "l[42314-42316]");
        assert_eq!(JobColumn::Reason.cell(&job), "Priority");

        // a job without a reason shows an empty cell
        job.reason = None;
        assert_eq!(JobColumn::Reason.cell(&job), "");
    }

    #[test]
    fn test_format_time() {
        let mut job = Job::new_default();
        job.time = "0-00:00:10".to_string();
        assert_eq!(format_time(&job), "00:00:10");
        job.time = "1-00:00:10".to_string();
        assert_eq!(format_time(&job), "1-00:00:10");
    }
}
