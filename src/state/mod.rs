extern crate serde_json;

mod graph;

use graph::{Graph};

use std::collections::HashMap;
use serde::{Serialize};
use std::time::{SystemTime, UNIX_EPOCH};
use super::{ReducedTelegram};

#[derive(Serialize, Debug, Clone)]
pub struct Tram {
    pub position_id: u32, // germany wide or local ones
    pub line: u32,
    pub run_number: u32,
    pub time_stamp: u64,
    pub delayed: i32,
    pub direction: u32
}


#[derive(Serialize, Debug, Clone)]
pub struct Line {
    pub trams: HashMap<u32, Tram>
}

#[derive(Serialize, Debug, Clone)]
pub struct Network {
    pub lines: HashMap<u32, Line>,
    pub positions: HashMap<u32, Vec<Tram>>,
    pub edges: HashMap<(u32, u32), u32>,
    pub graph: Graph
}

impl Network {
    pub fn new() -> Network {
        Network {
            lines: HashMap::new(),
            graph: Graph::from_file(&String::from("../graph.json")),
            positions: HashMap::new(),
            edges: HashMap::new()
        }
    }

    pub fn query_tram(&self, line: &u32, run_number: &u32) -> Option<u32> {
        match self.lines.get(line) {
            Some(line) => {
                line.trams.get(run_number).map_or(None, |tram| Some(tram.position_id))
            },
            None => None
        }
    }

    pub fn query_position(&mut self, position: &u32) -> Vec<Tram> {
        match self.positions.get(position) {
            Some(trams) => {
                trams.to_vec()
            },
            None => { Vec::new() }
        }
    }

    pub fn update(&mut self, telegram: &ReducedTelegram) {
        let new_tram = Tram {
            position_id: telegram.position_id,
            line: telegram.line,
            run_number: telegram.run_number,
            time_stamp: telegram.time_stamp,
            delayed: telegram.delay,
            direction: telegram.direction
        };

        match self.positions.get_mut(&telegram.position_id) {
            Some(trams) => {
                trams.push(new_tram.clone());
            }
            None => {
                self.positions.insert(telegram.position_id, vec![new_tram.clone()]);
            }
        }

        let mut start_time: u64;
        let mut remove_index = 0;
        match self.lines.get(&telegram.line) {
            Some(_)=> {
                {
                    let data = self.lines.get_mut(&telegram.line).unwrap();
                    data.trams.insert(telegram.run_number, new_tram.clone());
                }

                let mut previous = None;
                let possible_starts: Vec<u32> = self.graph.adjacent_paths(telegram.position_id);
                for start in possible_starts {
                    // we now look up if there is a tram started from this position

                    let trams = self.query_position(&start);
                    for (i ,found_tram) in trams.iter().enumerate() {
                        if found_tram.line == new_tram.line && found_tram.run_number == new_tram.run_number { // maybe add destination here
                            previous = Some(found_tram.clone());
                            remove_index = i;
                            break;
                            break;
                        }
                    }
                }

                if previous.is_some() {
                    let unwrapped = previous.unwrap();
                    let new_time = self.lines.get(&telegram.line).unwrap().trams.get(&telegram.run_number).unwrap().time_stamp;
                    let delta = unwrapped.time_stamp - new_time;
                    println!("Tram: Line: {} Run Number: {} followed path: {} -> {} Time: {}", unwrapped.line, unwrapped.run_number, unwrapped.position_id, telegram.position_id, delta);

                    self.positions.get_mut(&unwrapped.position_id).unwrap().remove(remove_index);
                }
            }
            None => {
                self.lines.insert(telegram.line, Line {trams: HashMap::from([(telegram.run_number, new_tram)])});
            }
        }
    }
}

pub struct State {
    pub regions: HashMap<u32, Network>
}


impl State {
    pub fn new() -> State {
        State {
            regions: HashMap::new()
        }
    }
}
