//! The display rows of the job table: job-array grouping.
//!
//! The job list itself stays a flat `Vec<Job>` (sorting, duplicate
//! removal, notifications and the scheduler know nothing about
//! grouping). [`build_rows`] derives the *display* rows from it: jobs
//! that are not array tasks become [`JobRow::Single`] rows, and two or
//! more tasks sharing an array base id (e.g. "12345_1", "12345_7",
//! "12345_[8-99]") collapse into one [`JobRow::Group`] header row.
//! Expanded groups additionally list their tasks as indented
//! [`JobRow::Task`] rows right below the header. The rows are a pure
//! function of the job list, the grouping option and the set of
//! expanded base ids, so they can be recomputed at any time without
//! going stale.

use std::collections::{HashMap, HashSet};

use crate::job::{array_base_id, parse_array_id, pending_range_count, ArrayTask, Job, JobStatus};

/// One display row of the job table. The indices point into the flat
/// job list the rows were built from.
#[derive(Debug, Clone, PartialEq)]
pub enum JobRow {
    /// A job that is not part of a displayed array group.
    Single { job_index: usize },
    /// The header row of a job-array group (>= 2 tasks with the same
    /// base id). The task indices are in job-list order.
    Group {
        base_id: String,
        task_indices: Vec<usize>,
        expanded: bool,
    },
    /// One task of an expanded group, rendered indented below the
    /// header. `last` marks the final task (for the tree glyph).
    Task { job_index: usize, last: bool },
}

/// Builds the display rows for the given job list.
///
/// With `group_arrays` disabled every job becomes a `Single` row. With
/// it enabled, jobs whose array base id occurs at least twice are
/// grouped: the group header is placed at the position of the first
/// task in job-list order (so groups stay contiguous under every sort
/// category), and base ids contained in `expanded` get their task rows
/// emitted below the header.
pub fn build_rows(jobs: &[Job], group_arrays: bool, expanded: &HashSet<String>) -> Vec<JobRow> {
    if !group_arrays {
        return (0..jobs.len())
            .map(|job_index| JobRow::Single { job_index })
            .collect();
    }
    // collect the task indices per base id, in job-list order
    let mut tasks_by_base: HashMap<&str, Vec<usize>> = HashMap::new();
    for (index, job) in jobs.iter().enumerate() {
        if let Some(base) = array_base_id(&job.id) {
            tasks_by_base.entry(base).or_default().push(index);
        }
    }
    let mut emitted: HashSet<&str> = HashSet::new();
    let mut rows = Vec::with_capacity(jobs.len());
    for (job_index, job) in jobs.iter().enumerate() {
        let base = array_base_id(&job.id);
        let group = base.and_then(|base| {
            let task_indices = tasks_by_base.get(base)?;
            // a lone array task is displayed like a plain job
            (task_indices.len() >= 2).then_some((base, task_indices))
        });
        let Some((base, task_indices)) = group else {
            rows.push(JobRow::Single { job_index });
            continue;
        };
        if !emitted.insert(base) {
            // the group header was already emitted at the first task
            continue;
        }
        let is_expanded = expanded.contains(base);
        rows.push(JobRow::Group {
            base_id: base.to_string(),
            task_indices: task_indices.clone(),
            expanded: is_expanded,
        });
        if is_expanded {
            let last_pos = task_indices.len() - 1;
            rows.extend(
                task_indices
                    .iter()
                    .enumerate()
                    .map(|(pos, &index)| JobRow::Task {
                        job_index: index,
                        last: pos == last_pos,
                    }),
            );
        }
    }
    rows
}

/// The compact aggregate status of a group's tasks, e.g. "3R 10PD 37CD".
///
/// The counts are listed in a fixed order (R, PD, CG, CD, F, TO, CA, ?)
/// and zero counts are omitted. A pending-range placeholder task like
/// "12345_[8-99]" counts as the number of tasks its range stands for.
pub fn status_counts(tasks: &[&Job]) -> String {
    const ORDER: [JobStatus; 8] = [
        JobStatus::Running,
        JobStatus::Pending,
        JobStatus::Completing,
        JobStatus::Completed,
        JobStatus::Failed,
        JobStatus::Timeout,
        JobStatus::Cancelled,
        JobStatus::Unknown,
    ];
    let mut counts = [0u64; ORDER.len()];
    for job in tasks {
        let weight = match parse_array_id(&job.id) {
            Some((_, ArrayTask::Range(range))) => pending_range_count(&range),
            _ => 1,
        };
        if let Some(position) = ORDER.iter().position(|status| status == &job.status) {
            counts[position] += weight;
        }
    }
    ORDER
        .iter()
        .zip(counts)
        .filter(|(_, count)| *count > 0)
        .map(|(status, count)| format!("{}{}", count, status.abbrev()))
        .collect::<Vec<_>>()
        .join(" ")
}

// ====================================================================
// TESTS
// ====================================================================

#[cfg(test)]
mod tests {
    use super::*;

    fn job(id: &str, status: JobStatus) -> Job {
        Job::new(
            id,
            &format!("job_{}", id),
            status,
            "00:10:00",
            "main",
            1,
            "/work",
            "cmd",
            None,
        )
    }

    /// A mixed list: two singles and two arrays (base 100 and 200).
    fn mixed_jobs() -> Vec<Job> {
        vec![
            job("42", JobStatus::Running),
            job("100_1", JobStatus::Running),
            job("100_2", JobStatus::Pending),
            job("7", JobStatus::Completed),
            job("200_0", JobStatus::Running),
            job("200_[5-9]", JobStatus::Pending),
        ]
    }

    #[test]
    fn grouping_disabled_yields_only_singles() {
        let jobs = mixed_jobs();
        let rows = build_rows(&jobs, false, &HashSet::new());
        assert_eq!(rows.len(), jobs.len());
        assert!(rows.iter().all(|row| matches!(row, JobRow::Single { .. })));
    }

    #[test]
    fn mixed_singles_and_two_arrays_are_grouped() {
        let jobs = mixed_jobs();
        let rows = build_rows(&jobs, true, &HashSet::new());
        assert_eq!(
            rows,
            vec![
                JobRow::Single { job_index: 0 },
                JobRow::Group {
                    base_id: "100".to_string(),
                    task_indices: vec![1, 2],
                    expanded: false,
                },
                JobRow::Single { job_index: 3 },
                JobRow::Group {
                    base_id: "200".to_string(),
                    task_indices: vec![4, 5],
                    expanded: false,
                },
            ]
        );
    }

    #[test]
    fn lone_array_task_stays_a_single_row() {
        // only one task of base 100: no group is formed
        let jobs = vec![
            job("42", JobStatus::Running),
            job("100_1", JobStatus::Running),
        ];
        let rows = build_rows(&jobs, true, &HashSet::new());
        assert_eq!(
            rows,
            vec![
                JobRow::Single { job_index: 0 },
                JobRow::Single { job_index: 1 },
            ]
        );
    }

    #[test]
    fn expanded_group_lists_its_tasks_below_the_header() {
        let jobs = mixed_jobs();
        let expanded: HashSet<String> = ["100".to_string()].into();
        let rows = build_rows(&jobs, true, &expanded);
        assert_eq!(
            rows,
            vec![
                JobRow::Single { job_index: 0 },
                JobRow::Group {
                    base_id: "100".to_string(),
                    task_indices: vec![1, 2],
                    expanded: true,
                },
                JobRow::Task {
                    job_index: 1,
                    last: false,
                },
                JobRow::Task {
                    job_index: 2,
                    last: true,
                },
                JobRow::Single { job_index: 3 },
                JobRow::Group {
                    base_id: "200".to_string(),
                    task_indices: vec![4, 5],
                    expanded: false,
                },
            ]
        );
    }

    #[test]
    fn scattered_tasks_form_a_contiguous_group_at_the_first_task() {
        // e.g. after sorting by status the tasks of one array may not
        // be adjacent in the flat list; the group must still be one row
        let jobs = vec![
            job("100_1", JobStatus::Running),
            job("42", JobStatus::Running),
            job("100_2", JobStatus::Pending),
        ];
        let rows = build_rows(&jobs, true, &HashSet::new());
        assert_eq!(
            rows,
            vec![
                JobRow::Group {
                    base_id: "100".to_string(),
                    task_indices: vec![0, 2],
                    expanded: false,
                },
                JobRow::Single { job_index: 1 },
            ]
        );
    }

    #[test]
    fn status_counts_aggregates_in_fixed_order() {
        let jobs = [
            job("100_1", JobStatus::Pending),
            job("100_2", JobStatus::Running),
            job("100_3", JobStatus::Completed),
            job("100_4", JobStatus::Running),
            job("100_5", JobStatus::Failed),
        ];
        let tasks: Vec<&Job> = jobs.iter().collect();
        assert_eq!(status_counts(&tasks), "2R 1PD 1CD 1F");
    }

    #[test]
    fn status_counts_expands_pending_range_placeholders() {
        // the placeholder "100_[8-99]" stands for 92 pending tasks
        let jobs = [
            job("100_1", JobStatus::Running),
            job("100_[8-99]", JobStatus::Pending),
        ];
        let tasks: Vec<&Job> = jobs.iter().collect();
        assert_eq!(status_counts(&tasks), "1R 92PD");
    }
}
