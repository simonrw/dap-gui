use anyhow::{Context, Result};
use eframe::egui;
use serde::{Deserialize, Serialize};
use std::future::{self, Future};
use std::task::Poll;
use std::io::{prelude::*, BufRead, BufReader};
use std::net;
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;

#[derive(Clone, Serialize, Default)]
struct InitializeArguments {
    #[serde(rename = "adapterID")]
    adapter_id: String,
}

#[derive(Clone, Serialize)]
#[serde(tag = "command")]
enum Command {
    #[serde(rename = "initialize")]
    Initialize(InitializeArguments),
}

#[derive(Serialize)]
struct Request {
    seq: usize,
    #[serde(rename = "type")]
    typ: String,
    #[serde(flatten)]
    command: Command,
}

impl Request {
    fn new(command: Command) -> Self {
        Request {
            seq: 1,
            typ: "request".to_string(),
            command,
        }
    }
}

#[derive(Debug, Deserialize)]
struct InitializeResponse {
    #[serde(rename = "supportsFunctionBreakpoints")]
    supports_function_breakpoints: bool,
}

#[derive(Debug, Deserialize)]
#[serde(tag = "command", content = "body")]
enum ResponseBody {
    #[serde(rename = "initialize")]
    Initialize(InitializeResponse),
}

#[derive(Debug, Deserialize)]
struct Response {
    seq: usize,
    #[serde(rename = "type")]
    typ: String,
    request_seq: i64,
    success: bool,
    message: Option<String>,

    #[serde(flatten)]
    body: ResponseBody,
}

#[derive(Debug, Deserialize)]
struct OutputEventBody {
    category: Option<String>,
    output: String,
    group: Option<String>,
    data: Option<serde_json::Value>,
}

#[derive(Debug, Deserialize)]
#[serde(tag = "event", content = "body")]
enum EventBody {
    #[serde(rename = "output")]
    Output(OutputEventBody),
}

#[derive(Debug, Deserialize)]
struct Event {
    #[serde(rename = "type")]
    typ: String,

    #[serde(flatten)]
    body: EventBody,
}

fn decode_server_message(body: &[u8]) -> Result<serde_json::Value> {
    let response: serde_json::Value = serde_json::from_slice(body).context("invalid JSON body")?;

    Ok(response)
}

#[derive(Default, Debug)]
struct Header {
    content_length: usize,
}

fn parse_header(header_s: &str) -> Result<Header> {
    let mut h = Header::default();

    for line in header_s.lines() {
        let mut parts = line.split(":");
        let key = parts.next().unwrap().trim();
        let value_s = parts.next().unwrap().trim();

        match key {
            "Content-Length" => {
                h.content_length = value_s.parse().unwrap();
            }
            _ => todo!("{key}"),
        }
    }

    Ok(h)
}

fn read_thread(mut stream: impl BufRead) -> Result<()> {
    loop {
        // read content length
        let header = {
            let mut buf = String::new();
            stream.read_line(&mut buf).context("reading header")?;

            let header = parse_header(&buf).context("parsing header")?;

            buf.clear();
            stream.read_line(&mut buf).context("reading newline")?;
            header
        };

        let mut buf = vec![0u8; header.content_length];
        stream.read_exact(&mut buf).context("reading body")?;

        match decode_server_message(&buf) {
            Ok(value) => {
                let response_type = value
                    .as_object()
                    .unwrap()
                    .get("type")
                    .unwrap()
                    .as_str()
                    .unwrap();
                match response_type {
                    "response" => {
                        eprintln!("decoding response");
                        let response: Response = serde_json::from_value(value).unwrap();
                        dbg!(response);
                    }
                    "event" => {
                        eprintln!("decoding event");
                        let event: Event = serde_json::from_value(value).unwrap();
                        dbg!(event);
                    }
                    _ => todo!("{response_type}"),
                }
            }
            Err(e) => eprintln!("error decoding response: {:?}", e),
        }
    }
}

fn serialise_message(msg: &Request) -> Result<String> {
    let body_ser = serde_json::to_string(msg).unwrap();
    // TODO: utf8 characters vs bytes
    let content_length = body_ser.len();

    let msg = format!("Content-Length: {content_length}\r\n\r\n{body_ser}");
    Ok(msg)
}

fn send_message<W>(mut w: W, req: &Request) -> Result<()>
where
    W: Write,
{
    let msg = serialise_message(req).context("serialising request")?;
    eprintln!("sending message {msg:?}");

    write!(w, "{msg}").context("sending message")?;

    Ok(())
}

struct Foo {
    values: Vec<i32>,
}

impl Foo {
    fn foo(&mut self) -> impl Future<Output = i32> + '_ {
        future::poll_fn(|_cx| -> Poll<i32> {
            for value in &self.values {
                if *value == 2 {
                    return Poll::Ready(*value);
                } 
            }

            Poll::Pending
        })
    }
}

#[tokio::main]
async fn main() {
    let mut f = Foo { values: vec![1, 2, 3] };
    let res = f.foo().await;
    dbg!(res);
}

fn foo_main() -> Result<()> {
    let conn = net::TcpStream::connect("127.0.0.1:5678").context("connecting to DAP server")?;
    let receiver = BufReader::new(conn.try_clone().context("cloning read socket handle")?);

    let th = std::thread::spawn(|| {
        let _ = read_thread(receiver);
    });

    let cmd = Command::Initialize(InitializeArguments {
        adapter_id: "dap-gui".to_string(),
        ..Default::default()
    });
    let req = Request::new(cmd);

    send_message(conn, &req).context("sending message")?;

    let _ = th.join();

    Ok(())
}

fn egui_main() -> Result<(), eframe::Error> {
    let options = eframe::NativeOptions {
        initial_window_size: Some(egui::vec2(320.0, 240.0)),
        ..Default::default()
    };
    eframe::run_native(
        "My egui App",
        options,
        Box::new(|cc| Box::new(MyApp::new(cc.egui_ctx.clone()))),
    )
}

struct MyApp {
    value: Arc<Mutex<u32>>,
    handle: thread::JoinHandle<()>,
    counter: u32,
}

impl MyApp {
    fn new(ctx: egui::Context) -> Self {
        let value = Arc::new(Mutex::new(0));
        let background_value = value.clone();
        let handle = thread::spawn(move || loop {
            *background_value.lock().unwrap() += 1;
            thread::sleep(Duration::from_secs(1));
            ctx.request_repaint();
        });
        Self {
            value,
            handle,
            counter: 0,
        }
    }
}

impl eframe::App for MyApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        println!("Updating {}", self.counter);
        self.counter += 1;
        egui::CentralPanel::default().show(ctx, |ui| {
            ui.heading("My egui Application");
            ui.label(format!("Value: {}", self.value.lock().unwrap()));
        });
    }
}
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn serialization() {
        let cmd = Command::Initialize(InitializeArguments {
            adapter_id: "dap-gui".to_string(),
        });
        let request = Request::new(cmd);
        let serialised = serialise_message(&request).unwrap();
        assert_eq!(serialised, "Content-Length: 71\r\n\r\n{\"seq\":1,\"type\":\"request\",\"command\":\"initialize\",\"adapterID\":\"dap-gui\"}");
    }
}
