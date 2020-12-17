#![feature(proc_macro_hygiene, decl_macro)]

#[macro_use]
extern crate rocket;
#[macro_use]
extern crate rocket_contrib;
#[macro_use]
extern crate serde_derive;
#[macro_use]
extern crate lazy_static;

use chashmap::CHashMap;
use envmnt::{ExpandOptions, ExpansionType};
use global::Global;
use nanoid::nanoid;
use rocket_contrib::json::{Json, JsonValue};
use rocket::request;

type Id = String;

static MAX_PENDING_RUNS: Global<u64> = Global::new();
static MAX_CONCURRENT_RUNS: Global<u64> = Global::new();

#[derive(Serialize, Deserialize)]
struct Client {
    status: String,
    successful_responses_count: u64,
    sum: i64,
}

impl Client {
    fn new() -> Client {
        Client {
            status: "IN_PROGRESS".to_string(),
            successful_responses_count: 0,
            sum: 0,
        }
    }
}

lazy_static! {
    static ref CLIENTS: CHashMap<Id, Client> = {
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

    let client = Client::new();
    CLIENTS.insert(id.clone(), client);

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

fn main() {
    *MAX_PENDING_RUNS.lock_mut().unwrap() = envmnt::get_u64("MAX_PENDING_RUNS", 5);
    *MAX_CONCURRENT_RUNS.lock_mut().unwrap() = envmnt::get_u64("MAX_CONCURRENT_RUNS", 5);

    println!("MAX_PENDING_RUNS = {}", *MAX_PENDING_RUNS.lock().unwrap());
    println!(
        "MAX_CONCURRENT_RUNS = {}",
        *MAX_CONCURRENT_RUNS.lock().unwrap()
    );

    rocket::ignite().mount("/", routes![run, get]).launch();
}
