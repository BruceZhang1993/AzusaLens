use std::{
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
    },
    thread,
};

use azusa_capture::{CapturedFrame, RegionCapture, begin_region_capture};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum CaptureOrigin {
    MainWindow,
    Background,
}

#[derive(Debug)]
pub(crate) enum CaptureEvent {
    SelectionReady {
        origin: CaptureOrigin,
        anchor_x: i32,
        anchor_y: i32,
        frame: CapturedFrame,
    },
    Failed {
        origin: CaptureOrigin,
        error: String,
    },
}

#[derive(Debug)]
struct PendingSelection {
    origin: CaptureOrigin,
    frame: CapturedFrame,
}

#[derive(Clone, Debug, Default)]
pub(crate) struct CaptureController {
    active: Arc<AtomicBool>,
    origin: Arc<Mutex<Option<CaptureOrigin>>>,
    pending: Arc<Mutex<Option<PendingSelection>>>,
}

impl CaptureController {
    pub(crate) fn is_active(&self) -> bool {
        self.active.load(Ordering::Acquire)
    }

    pub(crate) fn start<F>(&self, origin: CaptureOrigin, on_event: F) -> Result<bool, String>
    where
        F: FnOnce(CaptureEvent) + Send + 'static,
    {
        if self.active.swap(true, Ordering::AcqRel) {
            return Ok(false);
        }

        *self
            .origin
            .lock()
            .expect("capture origin state mutex poisoned") = Some(origin);
        *self
            .pending
            .lock()
            .expect("capture pending state mutex poisoned") = None;

        let active = Arc::clone(&self.active);
        let worker = thread::Builder::new()
            .name("azusa-capture-worker".to_owned())
            .spawn(move || {
                let event = match begin_region_capture() {
                    Ok(RegionCapture::NeedsSelection(selection)) => {
                        let (anchor_x, anchor_y) = selection.anchor();
                        CaptureEvent::SelectionReady {
                            origin,
                            anchor_x,
                            anchor_y,
                            frame: selection.into_frame(),
                        }
                    }
                    Err(error) => {
                        active.store(false, Ordering::Release);
                        CaptureEvent::Failed {
                            origin,
                            error: error.to_string(),
                        }
                    }
                };

                if slint::invoke_from_event_loop(move || on_event(event)).is_err() {
                    active.store(false, Ordering::Release);
                }
            });

        if let Err(error) = worker {
            self.reset();
            return Err(error.to_string());
        }

        Ok(true)
    }

    pub(crate) fn store_selection(&self, origin: CaptureOrigin, frame: CapturedFrame) {
        *self
            .origin
            .lock()
            .expect("capture origin state mutex poisoned") = Some(origin);
        *self
            .pending
            .lock()
            .expect("capture pending state mutex poisoned") = Some(PendingSelection { origin, frame });
    }

    pub(crate) fn take_selection(&self) -> Option<(CaptureOrigin, CapturedFrame)> {
        let selection = self
            .pending
            .lock()
            .expect("capture pending state mutex poisoned")
            .take();
        self.active.store(false, Ordering::Release);
        selection.map(|selection| (selection.origin, selection.frame))
    }

    pub(crate) fn cancel_selection(&self) -> Option<CaptureOrigin> {
        *self
            .pending
            .lock()
            .expect("capture pending state mutex poisoned") = None;
        self.active.store(false, Ordering::Release);
        self.origin
            .lock()
            .expect("capture origin state mutex poisoned")
            .take()
    }

    pub(crate) fn reset(&self) {
        *self
            .origin
            .lock()
            .expect("capture origin state mutex poisoned") = None;
        *self
            .pending
            .lock()
            .expect("capture pending state mutex poisoned") = None;
        self.active.store(false, Ordering::Release);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn selection_state_round_trips_and_preserves_origin_until_session_cancel() {
        let controller = CaptureController::default();
        controller.active.store(true, Ordering::Release);
        controller.store_selection(
            CaptureOrigin::Background,
            CapturedFrame::new(1, 1, vec![0, 0, 0, 255]).unwrap(),
        );

        let (origin, frame) = controller.take_selection().unwrap();
        assert_eq!(origin, CaptureOrigin::Background);
        assert_eq!((frame.width(), frame.height()), (1, 1));
        assert!(!controller.is_active());
        assert!(controller.take_selection().is_none());
        assert_eq!(controller.cancel_selection(), Some(CaptureOrigin::Background));
    }
}
