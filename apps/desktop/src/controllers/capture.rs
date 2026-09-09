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
    pending: Arc<Mutex<Option<PendingSelection>>>,
}

impl CaptureController {
    pub(crate) fn is_active(&self) -> bool {
        self.active.load(Ordering::Acquire)
    }

    pub(crate) fn start<F>(&self, origin: CaptureOrigin, on_event: F) -> Result<bool, String>
    where
        F: Fn(CaptureEvent) + Send + Sync + 'static,
    {
        if self.active.swap(true, Ordering::AcqRel) {
            return Ok(false);
        }

        *self
            .pending
            .lock()
            .expect("capture pending state mutex poisoned") = None;

        let active = Arc::clone(&self.active);
        let on_event = Arc::new(on_event);
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

                let callback = Arc::clone(&on_event);
                if slint::invoke_from_event_loop(move || callback(event)).is_err() {
                    active.store(false, Ordering::Release);
                }
            });

        if let Err(error) = worker {
            self.active.store(false, Ordering::Release);
            return Err(error.to_string());
        }

        Ok(true)
    }

    pub(crate) fn store_selection(
        &self,
        origin: CaptureOrigin,
        frame: CapturedFrame,
    ) {
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
        let selection = self
            .pending
            .lock()
            .expect("capture pending state mutex poisoned")
            .take();
        self.active.store(false, Ordering::Release);
        selection.map(|selection| selection.origin)
    }

    pub(crate) fn reset(&self) {
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
    fn selection_state_round_trips_and_resets_active_flag() {
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
    }
}
