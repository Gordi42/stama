
pub mod job_overview;
pub mod user_options_menu;
pub mod help;
pub mod job_allocation;
pub mod job_actions;
pub mod message;
pub mod confirmation;

#[derive(Debug, Clone)]
pub enum OpenMenu {
    JobOverview,
    UserOptions,
    Help(usize),
    JobAllocation,
    JobActions,
    Message(message::Message),
    Confirmation,
}

