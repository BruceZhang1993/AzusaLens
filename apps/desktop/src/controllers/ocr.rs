use std::{
    path::PathBuf,
    sync::{Arc, Mutex, mpsc},
    thread,
};

use azusa_ocr::{
    OcrDownloadCancellation, OcrEngine, OcrError, OcrImage, OcrModelManager, OcrResult,
    OcrTaskKind, create_engine,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum OcrModelAction {
    Install,
    Remove,
}

#[derive(Debug)]
pub(crate) enum OcrActionResult {
    Succeeded,
    Cancelled(String),
    Failed(String),
}

#[derive(Debug)]
pub(crate) enum OcrEvent {
    Recognition {
        epoch: i32,
        task: OcrTaskKind,
        result: Result<OcrResult, String>,
    },
    FileRecognition {
        task: OcrTaskKind,
        source_path: PathBuf,
        result: Result<OcrResult, String>,
    },
    ModelProgress {
        model_id: String,
        fraction: f32,
        detail: String,
    },
    ModelAction {
        model_id: String,
        action: OcrModelAction,
        result: OcrActionResult,
    },
}

enum OcrCommand {
    Recognize {
        epoch: i32,
        task: OcrTaskKind,
        model_id: String,
        image: OcrImage,
    },
    RecognizeFile {
        task: OcrTaskKind,
        model_id: String,
        source_path: PathBuf,
    },
    InstallModel {
        model_id: String,
        cancellation: OcrDownloadCancellation,
    },
    RemoveModel {
        model_id: String,
    },
}

type EventHandler = Arc<Mutex<Box<dyn Fn(OcrEvent) + Send + 'static>>>;

#[derive(Clone)]
pub(crate) struct OcrController {
    command_tx: mpsc::Sender<OcrCommand>,
    active_download: Arc<Mutex<Option<OcrDownloadCancellation>>>,
}

impl OcrController {
    pub(crate) fn new<F>(manager: OcrModelManager, on_event: F) -> Self
    where
        F: Fn(OcrEvent) + Send + 'static,
    {
        let (command_tx, command_rx) = mpsc::channel::<OcrCommand>();
        let active_download = Arc::new(Mutex::new(None::<OcrDownloadCancellation>));
        let worker_active_download = Arc::clone(&active_download);
        let on_event: EventHandler = Arc::new(Mutex::new(Box::new(on_event)));

        thread::Builder::new()
            .name("azusa-ocr-worker".to_owned())
            .spawn(move || {
                let mut active_engine: Option<(String, Box<dyn OcrEngine>)> = None;
                while let Ok(command) = command_rx.recv() {
                    match command {
                        OcrCommand::Recognize {
                            epoch,
                            task,
                            model_id,
                            image,
                        } => {
                            let result = with_engine(&mut active_engine, &model_id, |engine| {
                                engine.recognize_task(task, &image)
                            })
                            .map_err(|error| error.to_string());
                            dispatch_event(
                                &on_event,
                                OcrEvent::Recognition {
                                    epoch,
                                    task,
                                    result,
                                },
                            );
                        }
                        OcrCommand::RecognizeFile {
                            task,
                            model_id,
                            source_path,
                        } => {
                            let result = OcrImage::open_path(&source_path)
                                .and_then(|image| {
                                    with_engine(&mut active_engine, &model_id, |engine| {
                                        engine.recognize_task(task, &image)
                                    })
                                })
                                .map_err(|error| error.to_string());
                            dispatch_event(
                                &on_event,
                                OcrEvent::FileRecognition {
                                    task,
                                    source_path,
                                    result,
                                },
                            );
                        }
                        OcrCommand::InstallModel {
                            model_id,
                            cancellation,
                        } => {
                            let progress_handler = Arc::clone(&on_event);
                            let progress_model_id = model_id.clone();
                            let result = manager.install_model_with_progress(
                                &model_id,
                                &cancellation,
                                move |progress| {
                                    dispatch_event(
                                        &progress_handler,
                                        OcrEvent::ModelProgress {
                                            model_id: progress_model_id.clone(),
                                            fraction: progress.fraction.unwrap_or(-1.0),
                                            detail: progress.detail,
                                        },
                                    );
                                },
                            );
                            *worker_active_download
                                .lock()
                                .expect("OCR download state mutex poisoned") = None;
                            dispatch_event(
                                &on_event,
                                OcrEvent::ModelAction {
                                    model_id,
                                    action: OcrModelAction::Install,
                                    result: map_action_result(result),
                                },
                            );
                        }
                        OcrCommand::RemoveModel { model_id } => {
                            if active_engine
                                .as_ref()
                                .map(|(current_id, _)| current_id.as_str())
                                == Some(model_id.as_str())
                            {
                                active_engine = None;
                            }
                            let result = manager.remove_model(&model_id);
                            dispatch_event(
                                &on_event,
                                OcrEvent::ModelAction {
                                    model_id,
                                    action: OcrModelAction::Remove,
                                    result: map_action_result(result),
                                },
                            );
                        }
                    }
                }
            })
            .expect("failed to start local OCR worker");

        Self {
            command_tx,
            active_download,
        }
    }

    pub(crate) fn recognize(
        &self,
        epoch: i32,
        task: OcrTaskKind,
        model_id: String,
        image: OcrImage,
    ) -> Result<(), String> {
        self.command_tx
            .send(OcrCommand::Recognize {
                epoch,
                task,
                model_id,
                image,
            })
            .map_err(|error| error.to_string())
    }

    pub(crate) fn recognize_file(
        &self,
        task: OcrTaskKind,
        model_id: String,
        source_path: PathBuf,
    ) -> Result<(), String> {
        self.command_tx
            .send(OcrCommand::RecognizeFile {
                task,
                model_id,
                source_path,
            })
            .map_err(|error| error.to_string())
    }

    pub(crate) fn install_model(&self, model_id: String) -> Result<(), String> {
        let cancellation = OcrDownloadCancellation::new();
        *self
            .active_download
            .lock()
            .expect("OCR download state mutex poisoned") = Some(cancellation.clone());

        if let Err(error) = self.command_tx.send(OcrCommand::InstallModel {
            model_id,
            cancellation,
        }) {
            *self
                .active_download
                .lock()
                .expect("OCR download state mutex poisoned") = None;
            return Err(error.to_string());
        }
        Ok(())
    }

    pub(crate) fn cancel_model_download(&self) -> bool {
        let cancellation = self
            .active_download
            .lock()
            .expect("OCR download state mutex poisoned")
            .clone();
        if let Some(cancellation) = cancellation {
            cancellation.cancel();
            true
        } else {
            false
        }
    }

    pub(crate) fn remove_model(&self, model_id: String) -> Result<(), String> {
        self.command_tx
            .send(OcrCommand::RemoveModel { model_id })
            .map_err(|error| error.to_string())
    }
}

fn with_engine<T>(
    active_engine: &mut Option<(String, Box<dyn OcrEngine>)>,
    model_id: &str,
    action: impl FnOnce(&mut dyn OcrEngine) -> Result<T, OcrError>,
) -> Result<T, OcrError> {
    if active_engine
        .as_ref()
        .map(|(current_id, _)| current_id.as_str())
        != Some(model_id)
    {
        *active_engine = Some((model_id.to_owned(), create_engine(model_id)?));
    }
    action(
        active_engine
            .as_mut()
            .expect("OCR engine was initialized above")
            .1
            .as_mut(),
    )
}

fn map_action_result(result: Result<(), OcrError>) -> OcrActionResult {
    match result {
        Ok(()) => OcrActionResult::Succeeded,
        Err(OcrError::Cancelled(message)) => OcrActionResult::Cancelled(message),
        Err(error) => OcrActionResult::Failed(error.to_string()),
    }
}

fn dispatch_event(handler: &EventHandler, event: OcrEvent) {
    let handler = Arc::clone(handler);
    if let Err(error) = slint::invoke_from_event_loop(move || {
        let callback = handler.lock().expect("OCR event handler mutex poisoned");
        callback(event);
    }) {
        eprintln!("Could not dispatch OCR worker event to UI thread: {error}");
    }
}
