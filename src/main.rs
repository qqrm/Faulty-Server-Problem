#![feature(proc_macro_hygiene, decl_macro)]

#[macro_use]
extern crate rocket;
#[macro_use]
extern crate rocket_contrib;
#[macro_use]
extern crate serde_derive;
#[macro_use]
extern crate lazy_static;

extern crate tokio_timer;

use chashmap::CHashMap;
use envmnt::{ExpandOptions, ExpansionType};
use futures::Future;
// use futures::sync::mpsc;
use global::Global;
use nanoid::nanoid;
use reqwest::{header::HeaderMap, ClientBuilder};
use rocket_contrib::json::{Json, JsonValue};
use std::time::Duration;
use tokio::runtime::Runtime;
use tokio::sync::mpsc;
use tokio_timer::Timeout;

type Id = String;

static MAX_PENDING_RUNS: Global<u64> = Global::new();
static MAX_CONCURRENT_RUNS: Global<u64> = Global::new();

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

lazy_static! {
    static ref CLIENTS: CHashMap<Id, ClientStatus> = {
        let map = CHashMap::new();
        map
    };
}

#[derive(Serialize, Deserialize)]
struct StartParams {
    seconds: u64,
}

#[post("/runs", format = "application/json", data = "<start_params>")]
fn run(start_params: Json<StartParams>) -> JsonValue {
    println!("duration = {}", start_params.seconds);

    let id = nanoid!();

    let client = ClientStatus::new();
    CLIENTS.insert(id.clone(), client);

    let rt = tokio::runtime::Runtime::new().unwrap();
    rt.spawn(task(id.clone(), start_params.seconds));

    json!({ "id": id })
}

#[get("/runs/<id>")]
fn get(id: Id) -> JsonValue {
    match CLIENTS.get(&id) {
        Some(client) => json!({
          "status": (*client).status,
          "successful_responses_count": (*client).successful_responses_count,
          "sum": (*client).sum
        }),
        None => json!({ "err": "id not found" }),
    }
}

async fn task(id: Id, sec: u64) -> FutureObj<()> {
    println!("task starterd");

    let url = "http://faulty-server-htz-nbg1-1.wvservices.exchange:8080";
    let header = "X-Run-Id";

    // let (tx, rx) = tokio::sync::mpsc::channel();

    let process_req = async move {
        loop {
            let new_id = id.clone();
            println!("loop starterd");
            let client = reqwest::Client::new();
            let res = client
                .get(url)
                .header(header.clone(), new_id.clone())
                .send()
                .await;

            println!("res :{:?}", res.unwrap());

            // tx.send(res);
        }
    };

    process_req

    // let handle =
    // tokio::runtime::Runtime::.unwrap().spawn(process_req);

    // Timeout::new(handle, Duration::from_millis(10000));

    // Wrap the future with a `Timeout` set to expire in 10 milliseconds.

    // tokio::runtime::Runtime::new().unwrap().spawn(process_req);

    // process_req.timeout(Duration::from_millis(10));

    // let res = tokio::time::timeout(dur, j_h).await;
    // println!("{:?}", res);

    // let process = rx.for_each(|item| {
    //     println!("{:?}", item);
    // });

    // Wrap the future with a `Timeout` set to expire in 10 milliseconds.
}

fn main() {
    *MAX_PENDING_RUNS.lock_mut().unwrap() = envmnt::get_u64("MAX_PENDING_RUNS", 5);
    *MAX_CONCURRENT_RUNS.lock_mut().unwrap() = envmnt::get_u64("MAX_CONCURRENT_RUNS", 5);

    rocket::ignite().mount("/", routes![run, get]).launch();
}
