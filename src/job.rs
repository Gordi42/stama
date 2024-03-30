
pub enum JobStatus {
    Unknown,
    Running,
    Pending,
    Completed,
    Failed,
}

pub struct Job {
    pub id: u32,            // the job id
    pub name: String,       // the name of the job
    pub status: JobStatus,  // the status of the job
    pub time: u32,          // time in seconds
    pub partition: String,  // the partition the job is running on
    pub nodes: u32,         // the number of nodes the job is running on
}

// ====================================================================
//  CONSTRUCTOR
// ====================================================================

impl Job {
    pub fn new(id: u32, name: &str, status: JobStatus, time: u32, partition: &str, nodes: u32) -> Self {
        Self {
            id,
            name: name.to_string(),
            status,
            time,
            partition: partition.to_string(),
            nodes,
        }
    }
}



