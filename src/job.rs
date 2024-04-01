
#[derive(Debug, Clone, Default, PartialEq)]
pub enum JobStatus {
    #[default]
    Unknown,
    Running,
    Pending,
    Completing,
    Completed,
    Timeout,
    Cancelled,
    Failed,
}

impl JobStatus {
    pub fn priority(&self) -> usize {
        match self {
            JobStatus::Unknown => 0,
            JobStatus::Pending => 1,
            JobStatus::Running => 2,
            JobStatus::Failed => 3,
            JobStatus::Completing => 4,
            JobStatus::Completed => 5,
            JobStatus::Timeout => 6,
            JobStatus::Cancelled => 7,
        }
    }
}

#[derive(Debug, Clone, Default)]
pub struct Job {
    pub id: u32,            // the job id
    pub name: String,       // the name of the job
    pub status: JobStatus,  // the status of the job
    pub time: String,       // time string
    pub partition: String,  // the partition the job is running on
    pub nodes: u32,         // the number of nodes the job is running on
}

// ====================================================================
//  CONSTRUCTOR
// ====================================================================

impl Job {
    pub fn new(id: u32, name: &str, status: JobStatus, 
               time: &str, partition: &str, nodes: u32) -> Self {
        Self {
            id,
            name: name.to_string(),
            status: status,
            time: time.to_string(),
            partition: partition.to_string(),
            nodes: nodes,
        }
    }
}

// ====================================================================
// METHODS
// ====================================================================

impl Job {
    pub fn get_jobname(&self) -> String {
        self.name.clone()
    }
}



