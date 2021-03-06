use stop_names::TransmissionPosition;

use actix_web::{web, Responder};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::env;
use std::fs;

#[derive(Serialize, Deserialize)]
pub struct CoordinatesStation {
    pub station_id: u32,
}

#[derive(Serialize, Deserialize)]
pub struct Error {
    error_message: String,
}

// /static/{region}/coordinates
pub async fn coordinates(
    region: web::Path<String>,
    request: web::Json<CoordinatesStation>,
) -> impl Responder {
    // TODO: add the correct mapping
    let region_lookup: HashMap<&str, u32> = HashMap::from([
        ("dresden", 0),
        ("chemnitz", 1),
        ("karlsruhe", 2),
        ("berlin", 3),
    ]);

    let region_id;
    let default_stops = String::from("../stops.json");
    let stops_file = env::var("STOPS_FILE").unwrap_or(default_stops);

    println!("Reading File: {}", &stops_file);
    let data = fs::read_to_string(stops_file).expect("Unable to read file");
    let stops: HashMap<u32, HashMap<u32, TransmissionPosition>> =
        serde_json::from_str(&data).expect("Unable to parse");

    match region_lookup.get(&*region.as_str()) {
        Some(id) => {
            region_id = id;
        }
        None => {
            return web::Json(Err(Error {
                error_message: String::from("Invalid Region ID"),
            }));
        }
    };

    match stops.get(&region_id) {
        Some(station_look_up) => match station_look_up.get(&request.station_id) {
            Some(stop) => web::Json(Ok(stop.clone())),
            None => {
                return web::Json(Err(Error {
                    error_message: String::from("Station ID not found for region"),
                }))
            }
        },
        None => {
            return web::Json(Err(Error {
                error_message: String::from(
                    "This Server doesn't contain the config for this region",
                ),
            }))
        }
    }
}
