use std::sync::Arc;

use crate::executor::Handle;

struct TaskHandle(Arc<TaskInner>);

struct TaskInner {
    parent_handle: Handle,
}

// impl TaskHandle {
//     #[must_use]
//     pub(crate) fn new(handle: Handle) -> (Self, WaitTrigger) {
//         Self(inner)
//     }

//     #[must_use]
//     pub fn cancel(&self) -> Result<(), crate::event::WaitTokenError> {
//         self.0.cancel_token.trigger()
//     }
// }
