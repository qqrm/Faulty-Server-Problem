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
use reqwest::{ClientBuilder, StatusCode, header::HeaderMap};
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

#[derive(Serialize, Deserialize)]
struct IncomeNum {
    value: i32,
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

    let fut = task(data.clone(), id.clone(), start_params.seconds );

    actix_web::rt::spawn(fut);

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

async fn task(data: web::Data<AppState>, id: Id, seconds: u64) {
    println!("task starterd");

    let url = "http://faulty-server-htz-nbg1-1.wvservices.exchange:8080";
    let header = "X-Run-Id";

    let mut sum = &data.clients.get_mut(&id).unwrap();

    for i in 1..= seconds {
        println!("loop iter {}", i);
        let client = reqwest::Client::new();
        let res = client
            .get(url)
            .header(header.clone(), id.clone())
            .send()
            .await.unwrap();


        if res.status().is_success() {
            println!("res :{:?}", res);

            let js = res.json::<IncomeNum>().await.unwrap();
            println!("res :{:?}", js.value);
        }

        // let resp = res.unwrap().json::<IncomeNum>().await?;
      
        // println!("res :{:?}", resp);

        // let status = res.status();
        // match status {
        //     StatusCode::OK => {
        //         // sum = sum + = res.unwrap().json()
        //         println!("val :{:#?}", res.json()?)
        //     },
        //     _ =>  println!("other :{:?}", res)
        // }

        std::thread::sleep(Duration::from_secs(1)); //так?🤷‍♂️

    };
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
