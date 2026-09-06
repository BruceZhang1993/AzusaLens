use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};

use crate::OcrError;

/// Cooperative cancellation token shared between the UI and a model download worker.
#[derive(Debug, Clone, Default)]
pub struct OcrDownloadCancellation {
    cancelled: Arc<AtomicBool>,
}

impl OcrDownloadCancellation {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    pub fn cancel(&self) {
        self.cancelled.store(true, Ordering::Release);
    }

    #[must_use]
    pub fn is_cancelled(&self) -> bool {
        self.cancelled.load(Ordering::Acquire)
    }

    pub(crate) fn ensure_active(&self) -> Result<(), OcrError> {
        if self.is_cancelled() {
            Err(OcrError::Cancelled("model download was cancelled".to_owned()))
        } else {
            Ok(())
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct OcrModelDownloadProgress {
    /// `Some(0.0..=1.0)` when the backend can estimate progress, otherwise `None`.
    pub fraction: Option<f32>,
    pub detail: String,
}

impl OcrModelDownloadProgress {
    #[must_use]
    pub fn determinate(fraction: f32, detail: impl Into<String>) -> Self {
        Self {
            fraction: Some(fraction.clamp(0.0, 1.0)),
            detail: detail.into(),
        }
    }

    #[must_use]
    pub fn indeterminate(detail: impl Into<String>) -> Self {
        Self {
            fraction: None,
            detail: detail.into(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cancellation_is_shared_across_clones() {
        let cancellation = OcrDownloadCancellation::new();
        let worker = cancellation.clone();
        assert!(!worker.is_cancelled());
        cancellation.cancel();
        assert!(worker.is_cancelled());
        assert!(matches!(worker.ensure_active(), Err(OcrError::Cancelled(_))));
    }

    #[test]
    fn determinate_progress_is_clamped() {
        assert_eq!(
            OcrModelDownloadProgress::determinate(1.5, "done").fraction,
            Some(1.0)
        );
        assert_eq!(
            OcrModelDownloadProgress::determinate(-1.0, "start").fraction,
            Some(0.0)
        );
    }
}
