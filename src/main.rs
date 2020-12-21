#[macro_use]
extern crate serde_derive;
#[macro_use]
extern crate lazy_static;
#[macro_use]
extern crate actix;
#[macro_use]
extern crate actix_web;

use actix_web::{get, post, web, App, HttpResponse, HttpServer, Responder};
use envmnt::{ExpandOptions, ExpansionType};
use nanoid::nanoid;
use reqwest;
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::collections::{HashMap, VecDeque};
use std::sync::Mutex;
use std::time::Duration;
use web::Json;

type Id = String;

#[derive(Serialize, Deserialize)]
struct ClientStatus {
    status: Mutex<String>,
    successful_responses_count: u64,
    sum: Mutex<i64>,
}

impl ClientStatus {
    fn new() -> ClientStatus {
        ClientStatus {
            status: Mutex::new("NEW".to_string()),
            successful_responses_count: 0,
            sum: Mutex::new(0),
        }
    }

    pub fn add(&mut self, n: i64) {
        self.successful_responses_count += 1;
        *self.sum.get_mut().unwrap() += n;
    }

    fn set_status(&mut self, status: String) {
        *self.status.get_mut().unwrap() = status.to_string();
    }

    pub fn set_in_progress(&mut self) {
        self.set_status("IN_PROGRESS".to_string());
    }
    pub fn set_fin(&mut self) {
        self.set_status("FINISH".to_string());
    }
}

#[derive(Serialize, Deserialize)]
struct StartParams {
    seconds: u64,
}

#[derive(Serialize, Deserialize)]
struct IncomeNum {
    value: i32,
}

struct AppState {
    max_pend: Mutex<i64>,
    max_runs: Mutex<i64>,
    clients: Mutex<HashMap<Id, ClientStatus>>,
    current_clients: Mutex<i64>,
    current_clients_pends: Mutex<i64>,
    pends: Mutex<std::collections::VecDeque<(String, u64)>>,
}

#[post("/runs")]
async fn runs(data: web::Data<AppState>, start_params: Json<StartParams>) -> impl Responder {
    let id = nanoid!();

    let client = ClientStatus::new();
    data.clients.lock().unwrap().insert(id.clone(), client);

    if *data.current_clients.lock().unwrap() < *data.max_runs.lock().unwrap() {
        let fut = task(data.clone(), id.clone(), start_params.seconds);

        actix_web::rt::spawn(fut);

        return HttpResponse::Ok()
            .content_type("application/json")
            .body(json!({ "id": id }));
    } else if *data.current_clients_pends.lock().unwrap() < *data.max_pend.lock().unwrap() {
        *data.current_clients_pends.lock().unwrap() += 1;
        data.pends
            .lock()
            .unwrap()
            .push_back((id.clone(), start_params.seconds));

        return HttpResponse::Ok()
            .content_type("application/json")
            .body(json!({ "id": id }));
    } else {
        HttpResponse::TooManyRequests().finish()
    }
}

#[get("/runs/{id}")]
async fn run_info(data: web::Data<AppState>, path: web::Path<(String,)>) -> impl Responder {
    let id = path.into_inner().0;
    let js_resp = match data.clients.lock().unwrap().get(&id) {
        Some(client) => {
            json!({
                "status": (*client).status,
                "successful_responses_count": (*client).successful_responses_count,
                "sum": (*client).sum
            })
        }
        None => json!({ "err": "id not found" }),
    };

    HttpResponse::Ok()
        .content_type("application/json")
        .body(js_resp)
}

async fn task(data: web::Data<AppState>, id: Id, seconds: u64) {
    let url = "http://faulty-server-htz-nbg1-1.wvservices.exchange:8080";
    let header = "X-Run-Id";

    data.clients
        .lock()
        .unwrap()
        .get_mut(&id)
        .unwrap()
        .set_in_progress();
    *data.current_clients.lock().unwrap() += 1;

    for _ in 1..=seconds {
        let res = reqwest::Client::new()
            .get(url)
            .header(header.clone(), id.clone())
            .send()
            .await
            .unwrap();

        if res.status().is_success() {
            let js = res.json::<IncomeNum>().await.unwrap();
            data.clients
                .lock()
                .unwrap()
                .get_mut(&id)
                .unwrap()
                .add(js.value as i64);
        }

        std::thread::sleep(Duration::from_secs(1)); //так?🤷‍♂️
    }

    data.clients.lock().unwrap().get_mut(&id).unwrap().set_fin();
    *data.current_clients.lock().unwrap() -= 1;

    if data.pends.lock().unwrap().len() != 0 {
        *data.current_clients_pends.lock().unwrap() -= 1;
        let pend = data.pends.lock().unwrap().pop_front().unwrap();
        actix_web::rt::spawn(task(data, pend.0, pend.1));
    }
}

#[actix_web::main]
async fn main() -> std::io::Result<()> {
    let data = web::Data::new(AppState {
        max_pend: Mutex::new(envmnt::get_i64("MAX_PENDING_RUNS", 5)),
        max_runs: Mutex::new(envmnt::get_i64("MAX_CONCURRENT_RUNS", 5)),
        clients: Mutex::new(HashMap::new()),
        current_clients: Mutex::new(0),
        current_clients_pends: Mutex::new(0),
        pends: Mutex::new(std::collections::VecDeque::new()),
    });

    HttpServer::new(move || {
        App::new()
            .app_data(data.clone())
            .service(runs)
            .service(run_info)
    })
    .bind("127.0.0.1:8000")?
    .run()
    .await
}
