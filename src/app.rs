use crate::job_overview::JobOverview;

#[derive(Debug, Default)]
pub enum Action {
    #[default]
    None,
    Quit,
    OpenJobAction,
    OpenJobAllocation,
    OpenJobOverview,
}

pub struct App {
    pub action: Action,
    pub should_quit: bool,
    pub should_redraw: bool,
    pub job_overview: JobOverview,
}

impl App {
    pub fn new() -> Self {
        Self {
            action: Action::None,
            should_quit: false,
            should_redraw: true,
            job_overview: JobOverview::new(),
        }
    }

    pub fn quit(&mut self) {
        self.should_quit = true;
    }


    pub fn handle_action(&mut self) {
        match self.action {
            Action::Quit => { self.quit(); }
            _ => {}
        };
        self.action = Action::None;
    }

}
