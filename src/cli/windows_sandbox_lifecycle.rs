//! Staged lifecycle for one sandbox action. Pure state transitions only.
//! This is not a VM manager, guest transport, isolation guarantee or approval
//! implementation. Trusted callers must enforce and independently verify each
//! condition before delivering an event; guest messages are never evidence of
//! host-side isolation or teardown.

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum State {
    WaitingForApproval,
    Provisioning,
    WaitingForGuest,
    Running,
    Teardown,
    Finished,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum ResultStatus {
    Completed(i32),
    Denied,
    Cancelled,
    Failed,
    /// Action dispatch or destruction may have occurred without confirmation.
    Unknown,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Event {
    Approved,
    Denied,
    GuestProvisioned,
    GuestReady,
    GuestExited(i32),
    Cancel,
    Failure,
    DestroyConfirmed,
    DestroyUnconfirmed,
}

#[derive(Debug, PartialEq, Eq)]
pub(crate) enum TransitionError {
    InvalidTransition,
}

pub(crate) struct Lifecycle {
    state: State,
    result: Option<ResultStatus>,
}

impl Lifecycle {
    pub(crate) fn new() -> Self {
        Self {
            state: State::WaitingForApproval,
            result: None,
        }
    }

    pub(crate) fn state(&self) -> State {
        self.state
    }
    pub(crate) fn result(&self) -> Option<ResultStatus> {
        if self.state == State::Finished {
            self.result
        } else {
            None
        }
    }

    /// Invalid events leave the state unchanged. In particular, no completion
    /// is observable until the host verifies destruction of the exact guest.
    pub(crate) fn advance(&mut self, event: Event) -> Result<(), TransitionError> {
        use Event::*;
        use State::*;
        match (self.state, event) {
            (WaitingForApproval, Approved) => self.state = Provisioning,
            (WaitingForApproval, Denied | Cancel | Failure) => {
                self.result = Some(match event {
                    Denied => ResultStatus::Denied,
                    Cancel => ResultStatus::Cancelled,
                    _ => ResultStatus::Failed,
                });
                self.state = Finished;
            }
            (Provisioning, GuestProvisioned) => self.state = WaitingForGuest,
            (WaitingForGuest, GuestReady) => self.state = Running,
            (Running, GuestExited(code)) => {
                self.result = Some(ResultStatus::Completed(code));
                self.state = Teardown;
            }
            // Provisioning may have created a guest even if it failed. All
            // post-approval failures require teardown, never host execution.
            (Provisioning | WaitingForGuest, Cancel | Failure) => {
                self.result = Some(if event == Cancel {
                    ResultStatus::Cancelled
                } else {
                    ResultStatus::Failed
                });
                self.state = Teardown;
            }
            // Once execution may have started, a lost response or cancellation
            // cannot prove the action's outcome, even if cleanup succeeds.
            (Running, Cancel | Failure) => {
                self.result = Some(ResultStatus::Unknown);
                self.state = Teardown;
            }
            (Teardown, DestroyConfirmed) => self.state = Finished,
            (Teardown, DestroyUnconfirmed) => {
                self.result = Some(ResultStatus::Unknown);
                self.state = Finished;
            }
            _ => return Err(TransitionError::InvalidTransition),
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use Event::*;
    use State::*;

    #[test]
    fn success_is_not_finished_before_confirmed_destroy() {
        let mut run = Lifecycle::new();
        for event in [Approved, GuestProvisioned, GuestReady, GuestExited(0)] {
            run.advance(event).unwrap();
        }
        assert_eq!(run.state(), Teardown);
        assert_eq!(run.result(), None);
        run.advance(DestroyConfirmed).unwrap();
        assert_eq!(run.state(), Finished);
        assert_eq!(run.result(), Some(ResultStatus::Completed(0)));
        assert_eq!(
            run.advance(GuestReady),
            Err(TransitionError::InvalidTransition)
        );
    }

    #[test]
    fn out_of_order_guest_events_do_not_authorize_execution() {
        let mut run = Lifecycle::new();
        for event in [
            GuestReady,
            GuestExited(0),
            GuestProvisioned,
            DestroyConfirmed,
        ] {
            assert_eq!(run.advance(event), Err(TransitionError::InvalidTransition));
            assert_eq!(run.state(), WaitingForApproval);
        }
        run.advance(Denied).unwrap();
        assert_eq!(run.result(), Some(ResultStatus::Denied));
        assert_eq!(
            run.advance(Approved),
            Err(TransitionError::InvalidTransition)
        );
    }

    #[test]
    fn any_failure_after_approval_requires_cleanup_and_unknown_if_not_verified() {
        for prior in [
            vec![Approved],
            vec![Approved, GuestProvisioned],
            vec![Approved, GuestProvisioned, GuestReady],
        ] {
            let mut run = Lifecycle::new();
            for event in prior {
                run.advance(event).unwrap();
            }
            run.advance(Cancel).unwrap();
            assert_eq!(run.state(), Teardown);
            run.advance(DestroyUnconfirmed).unwrap();
            assert_eq!(run.result(), Some(ResultStatus::Unknown));
            assert_eq!(run.state(), Finished);
        }
    }

    #[test]
    fn running_cancellation_remains_unknown_even_after_confirmed_teardown() {
        let mut run = Lifecycle::new();
        for event in [
            Approved,
            GuestProvisioned,
            GuestReady,
            Cancel,
            DestroyConfirmed,
        ] {
            run.advance(event).unwrap();
        }
        assert_eq!(run.result(), Some(ResultStatus::Unknown));
    }

    #[test]
    fn uncertain_teardown_overrides_even_a_reported_success() {
        let mut run = Lifecycle::new();
        for event in [
            Approved,
            GuestProvisioned,
            GuestReady,
            GuestExited(0),
            DestroyUnconfirmed,
        ] {
            run.advance(event).unwrap();
        }
        assert_eq!(run.result(), Some(ResultStatus::Unknown));
    }
}
