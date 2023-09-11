use std::{
    io::{BufRead, BufReader},
    net::{TcpListener, TcpStream},
    sync::{
        mpsc::{self, Receiver},
        Arc, Mutex,
    },
    thread,
};

use dap_gui_client::events;
use eframe::egui;

#[cfg(feature = "sentry")]
macro_rules! setup_sentry {
    () => {
        log::info!("setting up sentry for crash reporting");
        let _guard = sentry::init((
            "https://f08b65bc9944ecbb855f1ebb2cadcb92@o366030.ingest.sentry.io/4505663159926784",
            sentry::ClientOptions {
                release: sentry::release_name!(),
                ..Default::default()
            },
        ));
    };
}

#[cfg(not(feature = "sentry"))]
macro_rules! setup_sentry {
    () => {};
}

#[derive(Debug)]
enum AppState {
    WaitingForConnection,
    Running,
}

impl AppState {
    fn render(&mut self, ctx: &egui::Context) {
        match self {
            AppState::WaitingForConnection => self.render_waiting_for_connection(ctx),
            AppState::Running => self.render_running(ctx),
        }
    }

    fn render_waiting_for_connection(&mut self, ctx: &egui::Context) {
        egui::CentralPanel::default().show(ctx, |ui| {
            ui.label("Waiting for connection");
        });
    }

    fn render_running(&mut self, ctx: &egui::Context) {
        egui::CentralPanel::default().show(ctx, |ui| {
            ui.label("Running");
        });
    }
}

#[allow(dead_code)]
struct MyApp {
    client: dap_gui_client::Client,
    app_state: Arc<Mutex<AppState>>,
}

fn control_worker(listener: TcpListener, state: Arc<Mutex<AppState>>) {
    thread::spawn(move || {
        listener.accept().into_iter().for_each(|(socket, _)| {
            // read instruction
            let mut reader = BufReader::new(socket);
            let mut line = String::new();
            reader.read_line(&mut line).unwrap();

            // update state accordingly
            let mut unlocked_state = state.lock().unwrap();
            *unlocked_state = AppState::Running;
            // TODO: trigger refresh
        });
    });
}

impl MyApp {
    pub fn new(client: dap_gui_client::Client, client_events: Receiver<events::Event>) -> Self {
        // set up background thread watching events

        let state = Arc::new(Mutex::new(AppState::WaitingForConnection));
        let trigger_socket =
            TcpListener::bind("127.0.0.1:8989").expect("could not bind control socket");
        let control_state = Arc::clone(&state);
        control_worker(trigger_socket, control_state);

        let background_state = Arc::clone(&state);
        let this = Self {
            client,
            app_state: state,
        };
        thread::spawn(move || {
            for event in client_events {
                handle_event(event, Arc::clone(&background_state));
            }
        });
        this
    }
}

fn handle_event(_event: events::Event, state_m: Arc<Mutex<AppState>>) {
    let _state = state_m.lock().unwrap();
}

impl eframe::App for MyApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        // TODO: locked for too long?
        let mut state = self.app_state.lock().unwrap();
        state.render(ctx);
    }
}

fn main() -> Result<(), eframe::Error> {
    setup_sentry!();

    let options = eframe::NativeOptions {
        initial_window_size: Some(egui::vec2(1024.0, 768.0)),
        ..Default::default()
    };
    let (tx, rx) = mpsc::channel();
    let stream = TcpStream::connect("127.0.0.1:5678").unwrap();
    let client = dap_gui_client::Client::new(stream, tx).unwrap();

    eframe::run_native(
        "DAP GUI",
        options,
        Box::new(move |_| Box::new(MyApp::new(client, rx))),
    )
}
