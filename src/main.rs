use actix_web::{get, post, web, App, HttpResponse, HttpServer, Responder};
use envmnt;
use nanoid::nanoid;
use reqwest;
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::collections::{HashMap, VecDeque};
use std::sync::Mutex;
use actix_rt::time::delay_for;

type Id = String;

/// Represents the status of a client.
#[derive(Serialize, Deserialize)]
struct ClientStatus {
    status: Mutex<String>,
    successful_responses_count: u64,
    sum: Mutex<i64>,
}

impl ClientStatus {
    /// Creates a new client status with default values.
    fn new() -> Self {
        ClientStatus {
            status: Mutex::new("NEW".to_string()),
            successful_responses_count: 0,
            sum: Mutex::new(0),
        }
    }

    /// Adds a value to the sum and increases the successful responses count.
    pub fn add(&mut self, n: i64) {
        self.successful_responses_count += 1;
        *self.sum.get_mut().expect("Failed to lock sum mutex") += n;
    }

    /// Sets the status of the client.
    fn set_status(&mut self, status: &str) {
        *self.status.get_mut().expect("Failed to lock status mutex") = status.to_string();
    }

    /// Marks the client status as in progress.
    pub fn set_in_progress(&mut self) {
        self.set_status("IN_PROGRESS");
    }

    /// Marks the client status as finished.
    pub fn set_fin(&mut self) {
        self.set_status("FINISH");
    }
}

/// Parameters required to start a run.
#[derive(Serialize, Deserialize)]
struct StartParams {
    seconds: u64,
}

/// Represents an income number.
#[derive(Serialize, Deserialize)]
struct IncomeNum {
    value: i32,
}

/// Contains application state data, including various counters and client statuses.
struct AppState {
    max_pend: Mutex<i64>,
    max_runs: Mutex<i64>,
    clients: Mutex<HashMap<Id, ClientStatus>>,
    current_clients: Mutex<i64>,
    current_clients_pends: Mutex<i64>,
    pends: Mutex<VecDeque<(String, u64)>>,
}

/// Endpoint to handle requests to start runs.
#[post("/runs")]
async fn runs(data: web::Data<AppState>, start_params: Json<StartParams>) -> impl Responder {
    let id = nanoid!();

    let client = ClientStatus::new();
    data.clients.lock().expect("Failed to lock clients mutex").insert(id.clone(), client);

    // Check if new tasks can be spawned or should be pending.
    if *data.current_clients.lock().expect("Failed to lock current_clients mutex") < *data.max_runs.lock().expect("Failed to lock max_runs mutex") {
        actix_web::rt::spawn(task(data.clone(), id.clone(), start_params.seconds));
        HttpResponse::Ok().content_type("application/json").body(json!({ "id": id }))
    } else if *data.current_clients_pends.lock().expect("Failed to lock current_clients_pends mutex") < *data.max_pend.lock().expect("Failed to lock max_pend mutex") {
        *data.current_clients_pends.lock().expect("Failed to lock current_clients_pends mutex") += 1;
        data.pends.lock().expect("Failed to lock pends mutex").push_back((id.clone(), start_params.seconds));
        HttpResponse::Ok().content_type("application/json").body(json!({ "id": id }))
    } else {
        HttpResponse::TooManyRequests().finish()
    }
}

/// Endpoint to fetch information about a specific run using its ID.
#[get("/runs/{id}")]
async fn run_info(data: web::Data<AppState>, path: web::Path<(String,)>) -> impl Responder {
    let id = path.into_inner().0;
    let clients = data.clients.lock().expect("Failed to lock clients mutex");
    match clients.get(&id) {
        Some(client) => {
            HttpResponse::Ok().content_type("application/json").body(json!({
                "status": *client.status.lock().expect("Failed to lock client status"),
                "successful_responses_count": client.successful_responses_count,
                "sum": *client.sum.lock().expect("Failed to lock client sum")
            }))
        }
        None => HttpResponse::Ok().content_type("application/json").body(json!({ "err": "id not found" }))
    }
}

/// Represents a task or run, which makes requests to an external server.
async fn task(data: web::Data<AppState>, id: Id, seconds: u64) {
    let url = "http://faulty-server-htz-nbg1-1.wvservices.exchange:8080";
    let header = "X-Run-Id";

    data.clients.lock().expect("Failed to lock clients mutex").get_mut(&id).expect("Client not found").set_in_progress();
    *data.current_clients.lock().expect("Failed to lock current_clients mutex") += 1;

    for _ in 1..=seconds {
        let res = reqwest::Client::new().get(url).header(header, &id).send().await.expect("Failed to send request");
        if res.status().is_success() {
            let js = res.json::<IncomeNum>().await.expect("Failed to deserialize IncomeNum");
            data.clients.lock().expect("Failed to lock clients mutex").get_mut(&id).expect("Client not found").add(js.value as i64);
        }
        delay_for(Duration::from_secs(1)).await;  // Asynchronous sleep
    }

    data.clients.lock().expect("Failed to lock clients mutex").get_mut(&id).
