use std::sync::{Arc, Mutex};

use crossbeam_channel::Sender;
use tiny_http::{Header, Method, Response, Server};

use crate::app::PlaybackState;

/// Remote control command sent from HTTP
pub enum RemoteCommand {
    Toggle,
    Next,
    Prev,
    SetVolume(f32),
    Seek(f64),
    CycleTheme,
    CycleVisualizer,
    ToggleShuffle,
}

pub struct RemoteServer {
    state: Arc<Mutex<PlaybackState>>,
    cmd_tx: Sender<RemoteCommand>,
}

impl RemoteServer {
    pub fn new(state: Arc<Mutex<PlaybackState>>, cmd_tx: Sender<RemoteCommand>) -> Self {
        Self { state, cmd_tx }
    }

    pub fn run(&self, port: u16) {
        let addr = format!("0.0.0.0:{}", port);
        let server = match Server::http(&addr) {
            Ok(s) => s,
            Err(e) => {
                eprintln!("Failed to start remote server: {}", e);
                return;
            }
        };

        for request in server.incoming_requests() {
            let url = request.url().to_string();
            let method = request.method().clone();

            let response = match (method, url.as_str()) {
                (Method::Get, "/") => self.serve_html(),
                (Method::Get, "/api/status") => self.get_status(),
                (Method::Post, "/api/toggle") => self.handle_toggle(),
                (Method::Post, "/api/next") => self.handle_next(),
                (Method::Post, "/api/prev") => self.handle_prev(),
                (Method::Post, "/api/theme") => self.handle_theme(),
                (Method::Post, "/api/visualizer") => self.handle_visualizer(),
                (Method::Post, "/api/shuffle") => self.handle_shuffle(),
                (Method::Post, path) if path.starts_with("/api/volume") => {
                    self.handle_volume(&url)
                }
                (Method::Post, path) if path.starts_with("/api/seek") => {
                    self.handle_seek(&url)
                }
                _ => Response::from_string("Not Found").with_status_code(404).boxed(),
            };

            let _ = request.respond(response);
        }
    }

    fn serve_html(&self) -> tiny_http::ResponseBox {
        let html = include_str!("remote.html");
        Response::from_string(html)
            .with_header(Header::from_bytes("Content-Type", "text/html; charset=utf-8").unwrap())
            .boxed()
    }

    fn get_status(&self) -> tiny_http::ResponseBox {
        let state = self.state.lock().unwrap().clone();
        let json = serde_json::to_string(&state).unwrap_or_else(|_| "{}".to_string());
        Response::from_string(json)
            .with_header(Header::from_bytes("Content-Type", "application/json").unwrap())
            .boxed()
    }

    fn handle_toggle(&self) -> tiny_http::ResponseBox {
        let _ = self.cmd_tx.send(RemoteCommand::Toggle);
        Response::from_string("OK").boxed()
    }

    fn handle_next(&self) -> tiny_http::ResponseBox {
        let _ = self.cmd_tx.send(RemoteCommand::Next);
        Response::from_string("OK").boxed()
    }

    fn handle_prev(&self) -> tiny_http::ResponseBox {
        let _ = self.cmd_tx.send(RemoteCommand::Prev);
        Response::from_string("OK").boxed()
    }

    fn handle_volume(&self, url: &str) -> tiny_http::ResponseBox {
        if let Some(v) = parse_query_param(url, "v") {
            if let Ok(vol) = v.parse::<f32>() {
                let vol = vol.clamp(0.0, 1.0);
                let _ = self.cmd_tx.send(RemoteCommand::SetVolume(vol));
                return Response::from_string("OK").boxed();
            }
        }
        Response::from_string("Bad Request").with_status_code(400).boxed()
    }

    fn handle_seek(&self, url: &str) -> tiny_http::ResponseBox {
        if let Some(t) = parse_query_param(url, "t") {
            if let Ok(time) = t.parse::<f64>() {
                let _ = self.cmd_tx.send(RemoteCommand::Seek(time));
                return Response::from_string("OK").boxed();
            }
        }
        Response::from_string("Bad Request").with_status_code(400).boxed()
    }

    fn handle_theme(&self) -> tiny_http::ResponseBox {
        let _ = self.cmd_tx.send(RemoteCommand::CycleTheme);
        Response::from_string("OK").boxed()
    }

    fn handle_visualizer(&self) -> tiny_http::ResponseBox {
        let _ = self.cmd_tx.send(RemoteCommand::CycleVisualizer);
        Response::from_string("OK").boxed()
    }

    fn handle_shuffle(&self) -> tiny_http::ResponseBox {
        let _ = self.cmd_tx.send(RemoteCommand::ToggleShuffle);
        Response::from_string("OK").boxed()
    }
}

fn parse_query_param(url: &str, key: &str) -> Option<String> {
    let query = url.split('?').nth(1)?;
    for pair in query.split('&') {
        let mut parts = pair.splitn(2, '=');
        if parts.next() == Some(key) {
            return parts.next().map(|s| s.to_string());
        }
    }
    None
}

/// Get local IP address for display
pub fn get_local_ip() -> Option<String> {
    use std::net::UdpSocket;
    let socket = UdpSocket::bind("0.0.0.0:0").ok()?;
    socket.connect("8.8.8.8:80").ok()?;
    socket.local_addr().ok().map(|a| a.ip().to_string())
}
