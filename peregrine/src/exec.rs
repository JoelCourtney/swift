use crate::History;
use crate::operation::ErrorAccumulator;

pub const STACK_LIMIT: usize = 1000;

#[derive(Copy, Clone)]
pub struct ExecEnvironment<'s, 'o: 's> {
    pub history: &'o History,
    pub errors: &'s ErrorAccumulator,
    pub stack_counter: usize,
}

impl<'s, 'o> ExecEnvironment<'s, 'o> {
    pub fn increment(self) -> ExecEnvironment<'s, 'o> {
        Self {
            stack_counter: self.stack_counter + 1,
            ..self
        }
    }

    pub fn reset(self) -> ExecEnvironment<'s, 'o> {
        Self {
            stack_counter: 0,
            ..self
        }
    }
}
