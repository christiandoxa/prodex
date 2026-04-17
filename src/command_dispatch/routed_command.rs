use super::*;

pub(super) trait CommandAction {
    fn run(self: Box<Self>) -> Result<()>;
}

pub(super) struct RoutedCommand {
    action: Box<dyn CommandAction>,
}

impl RoutedCommand {
    pub(super) fn new<T>(action: T) -> Self
    where
        T: CommandAction + 'static,
    {
        Self {
            action: Box::new(action),
        }
    }

    pub(super) fn execute(self) -> Result<()> {
        self.action.run()
    }
}
