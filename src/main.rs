#[macro_use]
extern crate serde_derive;
#[macro_use]
extern crate lazy_static;
#[macro_use]
extern crate actix;
#[macro_use]
extern crate actix_web;

use actix_web::{get, post, web, App, HttpResponse, HttpServer, Responder};
use chashmap::CHashMap;
use envmnt::{ExpandOptions, ExpansionType};
use nanoid::nanoid;
use reqwest::{header::HeaderMap, ClientBuilder};
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::sync::Mutex;
use std::time::Duration;
use web::Json;



type Id = String;

// static MAX_PENDING_RUNS: Global<u64> = Global::new();
// static MAX_CONCURRENT_RUNS: Global<u64> = Global::new();

#[derive(Serialize, Deserialize)]
struct ClientStatus {
    status: String,
    successful_responses_count: u64,
    sum: i64,
}

impl ClientStatus {
    fn new() -> ClientStatus {
        ClientStatus {
            status: "IN_PROGRESS".to_string(),
            successful_responses_count: 0,
            sum: 0,
        }
    }

    fn add(&mut self, n: i64) {
        self.successful_responses_count = self.successful_responses_count + 1;
        self.sum = self.sum + n;
    }
}

// lazy_static! {
//     static ref CLIENTS: CHashMap<Id, ClientStatus> = {
//         let map = CHashMap::new();
//         map
//     };
// }

#[derive(Serialize, Deserialize)]
struct StartParams {
    seconds: u64,
}

struct AppState {
    max_pend: Mutex<u64>,
    max_runs: Mutex<u64>,
    clients: CHashMap<Id, ClientStatus>,
}

#[post("/runs")]
async fn runs(data: web::Data<AppState>, start_params: Json<StartParams>) -> impl Responder {
    let id = nanoid!();

    let client = ClientStatus::new();
    &data.clients.insert(id.clone(), client);


    let fut = task(id.clone());
    // let res = actix::run(fut);

    actix_web::rt::spawn(fut);

    // println!("{:?}", res);


    // tokio::



    // let max_th = &data.max_runs;

    HttpResponse::Ok()
        .content_type("application/json")
        .body(json!({ "id": id }))
}

#[get("/runs/{id}")]
async fn run_info(data: web::Data<AppState>, id: Id) -> impl Responder {
    let js_resp = match &data.clients.get(&id) {
        Some(client) => json!({
          "status": (*client).status,
          "successful_responses_count": (*client).successful_responses_count,
          "sum": (*client).sum
        }),
        None => json!({ "err": "id not found" }),
    };

    HttpResponse::Ok()
        .content_type("application/json")
        .body(js_resp)
}

async fn task(id: Id) {
    println!("task starterd");

    let url = "http://faulty-server-htz-nbg1-1.wvservices.exchange:8080";
    let header = "X-Run-Id";

    // let (tx, rx) = tokio::sync::mpsc::channel();

    // loop {
        println!("loop starterd");
        let new_id = id.clone();
        let client = reqwest::Client::new();
        let res = client
            .get(url)
            .header(header.clone(), new_id.clone())
            .send()
            .await;

        println!("res :{:?}", res.unwrap());

        // tx.send(res);
    // }

    // let handle = tokio::runtime::Runtime::.unwrap().spawn(process_req);

    // Timeout::new(handle, Duration::from_millis(10000));

    // tokio::runtime::Runtime::new().unwrap().spawn(process_req);

    // process_req.timeout(Duration::from_millis(10));

    // let res = tokio::time::timeout(dur, j_h).await;
    // println!("{:?}", res);

    // let process = rx.for_each(|item| {
    //     println!("{:?}", item);
    // });

    // Wrap the future with a `Timeout` set to expire in 10 milliseconds.
}

#[actix_web::main]
async fn main() -> std::io::Result<()> {
    let data = web::Data::new(AppState {
        max_pend: Mutex::new(envmnt::get_u64("MAX_PENDING_RUNS", 5)),
        max_runs: Mutex::new(envmnt::get_u64("MAX_CONCURRENT_RUNS", 5)),
        clients: CHashMap::new(),
    });

    HttpServer::new(move || App::new()
    .app_data(data.clone())
    .service(runs)
    .service(run_info))
        .bind("127.0.0.1:8000")?
        .run()
        .await
}
