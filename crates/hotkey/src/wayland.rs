use std::{
    sync::{Mutex, OnceLock, mpsc},
    thread,
    time::Duration,
};

use ashpd::{
    AppID,
    desktop::{CreateSessionOptions, global_shortcuts::GlobalShortcuts},
};
use futures_util::StreamExt;
use tokio::{
    runtime::Builder,
    sync::oneshot,
    time::{self, error::Elapsed},
};
use wayclip_global_hotkey::hotkey::HotKey;

use crate::APP_ID;

const REGISTRATION_TIMEOUT: Duration = Duration::from_secs(10);
const REGISTRATION_GRACE_PERIOD: Duration = Duration::from_secs(1);
const SESSION_CLOSE_TIMEOUT: Duration = Duration::from_secs(2);

static HOST_APP_REGISTERED: OnceLock<()> = OnceLock::new();

struct PortalEvent {
    pressed: bool,
}

pub(crate) struct WaylandHotkey {
    events: Mutex<mpsc::Receiver<PortalEvent>>,
    shutdown: Option<oneshot::Sender<()>>,
}

impl WaylandHotkey {
    pub(crate) fn register(hotkey: HotKey) -> Result<Self, String> {
        let (ready_sender, ready_receiver) = mpsc::sync_channel(1);
        let (event_sender, event_receiver) = mpsc::channel();
        let (shutdown_sender, shutdown_receiver) = oneshot::channel();

        thread::Builder::new()
            .name("azusa-wayland-hotkey".to_owned())
            .spawn(move || {
                let runtime = match Builder::new_current_thread().enable_all().build() {
                    Ok(runtime) => runtime,
                    Err(error) => {
                        let _ = ready_sender.send(Err(format!(
                            "failed to start Wayland shortcut runtime: {error}"
                        )));
                        return;
                    }
                };

                runtime.block_on(run_portal_hotkey(
                    hotkey,
                    event_sender,
                    ready_sender,
                    shutdown_receiver,
                ));
            })
            .map_err(|error| format!("failed to start Wayland shortcut worker: {error}"))?;

        match ready_receiver.recv_timeout(REGISTRATION_TIMEOUT + REGISTRATION_GRACE_PERIOD) {
            Ok(Ok(())) => Ok(Self {
                events: Mutex::new(event_receiver),
                shutdown: Some(shutdown_sender),
            }),
            Ok(Err(error)) => Err(error),
            Err(mpsc::RecvTimeoutError::Timeout) => {
                let _ = shutdown_sender.send(());
                Err("Wayland shortcut portal timed out; check desktop portal permissions".into())
            }
            Err(mpsc::RecvTimeoutError::Disconnected) => {
                Err("Wayland shortcut worker disconnected".into())
            }
        }
    }

    pub(crate) fn take_pressed(&self) -> bool {
        let Ok(events) = self.events.lock() else {
            return false;
        };

        let mut pressed = false;
        while let Ok(event) = events.try_recv() {
            pressed |= event.pressed;
        }
        pressed
    }
}

impl Drop for WaylandHotkey {
    fn drop(&mut self) {
        if let Some(shutdown) = self.shutdown.take() {
            let _ = shutdown.send(());
        }
    }
}

async fn run_portal_hotkey(
    hotkey: HotKey,
    event_sender: mpsc::Sender<PortalEvent>,
    ready_sender: mpsc::SyncSender<Result<(), String>>,
    mut shutdown_receiver: oneshot::Receiver<()>,
) {
    let setup = async {
        let app_id = AppID::try_from(APP_ID)
            .map_err(|error| format!("invalid application id for shortcut portal: {error}"))?;

        if HOST_APP_REGISTERED.get().is_none() {
            match time::timeout(REGISTRATION_TIMEOUT, ashpd::register_host_app(app_id)).await {
                Ok(Ok(())) => {
                    let _ = HOST_APP_REGISTERED.set(());
                }
                Ok(Err(error)) => {
                    return Err(format!(
                        "desktop portal rejected the application id: {error}"
                    ));
                }
                Err(_) => {
                    return Err("desktop portal application registration timed out".to_owned());
                }
            }
        }

        let proxy = time::timeout(REGISTRATION_TIMEOUT, GlobalShortcuts::new())
            .await
            .map_err(timeout_error)?
            .map_err(|error| format!("failed to start global shortcuts portal proxy: {error}"))?;
        let session = time::timeout(
            REGISTRATION_TIMEOUT,
            proxy.create_session(CreateSessionOptions::default()),
        )
        .await
        .map_err(timeout_error)?
        .map_err(|error| format!("failed to start global shortcuts portal session: {error}"))?;
        let activated = time::timeout(REGISTRATION_TIMEOUT, proxy.receive_activated())
            .await
            .map_err(timeout_error)?
            .map_err(|error| format!("failed to receive shortcut activation events: {error}"))?;
        let deactivated = time::timeout(REGISTRATION_TIMEOUT, proxy.receive_deactivated())
            .await
            .map_err(timeout_error)?
            .map_err(|error| format!("failed to receive shortcut deactivation events: {error}"))?;

        let shortcut_id = hotkey.id().to_string();
        let shortcut = ashpd::desktop::global_shortcuts::NewShortcut::new(
            shortcut_id.clone(),
            "Azusa Lens screenshot",
        )
        .preferred_trigger(Some("Print"));
        let request = match time::timeout(
            REGISTRATION_TIMEOUT,
            proxy.bind_shortcuts(&session, &[shortcut], None, Default::default()),
        )
        .await
        .map_err(timeout_error)?
        .map_err(|error| format!("failed to request PrtSc registration: {error}"))
        {
            Ok(request) => request,
            Err(error) => {
                close_session(&session).await;
                return Err(error);
            }
        };

        let response = match request.response() {
            Ok(response) => response,
            Err(error) => {
                close_session(&session).await;
                return Err(format!(
                    "PrtSc registration was cancelled or rejected: {error}"
                ));
            }
        };

        if !response
            .shortcuts()
            .iter()
            .any(|shortcut| shortcut.id() == shortcut_id.as_str())
        {
            close_session(&session).await;
            return Err(
                "desktop portal completed registration but did not bind PrtSc; the key may already be occupied"
                    .to_owned(),
            );
        }

        Ok((proxy, session, activated, deactivated))
    };

    let setup_result = tokio::select! {
        result = time::timeout(REGISTRATION_TIMEOUT, setup) => result.map_err(timeout_error).and_then(|result| result),
        _ = &mut shutdown_receiver => Err("Wayland shortcut registration cancelled".to_owned()),
    };

    let (proxy, session, mut activated, mut deactivated) = match setup_result {
        Ok(result) => result,
        Err(error) => {
            let _ = ready_sender.send(Err(error));
            return;
        }
    };

    if ready_sender.send(Ok(())).is_err() {
        close_session(&session).await;
        return;
    }

    let _proxy = proxy;
    loop {
        tokio::select! {
            _ = &mut shutdown_receiver => break,
            event = activated.next() => {
                let Some(event) = event else { break; };
                if event.shortcut_id().parse::<u32>().ok() == Some(hotkey.id()) {
                    let _ = event_sender.send(PortalEvent { pressed: true });
                }
            }
            event = deactivated.next() => {
                let Some(event) = event else { break; };
                if event.shortcut_id().parse::<u32>().ok() == Some(hotkey.id()) {
                    let _ = event_sender.send(PortalEvent { pressed: false });
                }
            }
        }
    }

    close_session(&session).await;
}

async fn close_session<T>(session: &ashpd::desktop::Session<T>)
where
    T: ashpd::desktop::SessionPortal,
{
    let _ = time::timeout(SESSION_CLOSE_TIMEOUT, session.close()).await;
}

fn timeout_error(_: Elapsed) -> String {
    "Wayland shortcut portal timed out; check desktop portal permissions".to_owned()
}
