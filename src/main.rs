use std::cmp::Ordering;
use std::collections::{BinaryHeap, HashMap};
use std::error::Error;
use std::f32::consts::PI;
use std::fs::File;
use std::io::BufReader;
use std::path::Path;
use chrono::{NaiveDateTime};
use ordered_float::OrderedFloat;
use serde::{de, Deserialize, Deserializer, Serialize};

#[derive(Deserialize, Debug)]
struct Data {
    load_id: i32,
    origin_city: String,
    origin_state: String,
    origin_latitude: f64,
    origin_longitude: f64,
    destination_city: String,
    destination_state: String,
    destination_latitude: f64,
    destination_longitude: f64,
    amount: i32,
    #[serde(deserialize_with = "naive_date_time_from_data")]
    pickup_date_time: NaiveDateTime,
}

fn naive_date_time_from_data<'de, D>(deserializer: D) -> Result<NaiveDateTime, D::Error>
    where
        D: Deserializer<'de>,
{
    let s: String = Deserialize::deserialize(deserializer)?;
    NaiveDateTime::parse_from_str(&s, "%Y-%m-%dT%H:%M:%S.%fZ").map_err(de::Error::custom)
}

#[derive(Deserialize, Debug)]
struct Input {
    input_trip_id: i32,
    start_latitude: f64,
    start_longitude: f64,
    #[serde(deserialize_with = "naive_date_time_from_input")]
    start_time: NaiveDateTime,
    #[serde(deserialize_with = "naive_date_time_from_input")]
    max_destination_time: NaiveDateTime,
}

fn naive_date_time_from_input<'de, D>(deserializer: D) -> Result<NaiveDateTime, D::Error>
    where
        D: Deserializer<'de>,
{
    let s: String = Deserialize::deserialize(deserializer)?;
    NaiveDateTime::parse_from_str(&s, "%Y-%m-%d %H:%M:%S").map_err(de::Error::custom)
}

#[derive(Serialize)]
struct Output {
    input_trip_id: i32,
    load_ids: Vec<i32>,
}

const MILES_TRAVELLED_PER_HOUR: i32 = 55;
const FUEL_COST_PER_MILE: f64 = 0.40;

fn get_geodesic_distance(start: (f64, f64), end: (f64, f64)) -> f64 {
    const RADIUS_OF_EARTH: i32 = 6371000; // metres
    const DEGREES_TO_RADIANS: f64 = std::f64::consts::PI / 180.;
    const METRES_TO_MILES: f64 = 1. / 1609.344;

    let theta_1 = start.0 * DEGREES_TO_RADIANS;
    let theta_2 = end.0 * DEGREES_TO_RADIANS;
    let delta_theta = (end.0 - start.0) * DEGREES_TO_RADIANS;
    let delta_lambda = (end.1 - start.1) * DEGREES_TO_RADIANS;

    let a = (delta_theta / 2.).sin().powf(2.) + theta_1.cos() * theta_2.cos() * (delta_lambda / 2.).sin().powf(2.);
    let c = 2. * a.sqrt().atan2((1. - a).sqrt());

    RADIUS_OF_EARTH as f64 * c * METRES_TO_MILES
}

fn load_data<P: AsRef<Path>>(path: P) -> Result<Vec<Data>, Box<dyn Error>> {
    let file = File::open(path)?;
    let reader = BufReader::new(file);

    let data: Vec<Data> = serde_json::from_reader(reader)?;

    Ok(data)
}

fn load_input<P: AsRef<Path>>(path: P) -> Result<Vec<Input>, Box<dyn Error>> {
    let file = File::open(path)?;
    let reader = BufReader::new(file);

    let data: Vec<Input> = serde_json::from_reader(reader)?;

    Ok(data)
}

#[derive(Debug)]
struct Node {
    location: (OrderedFloat<f64>, OrderedFloat<f64>),

    parent: Option<(OrderedFloat<f64>, OrderedFloat<f64>)>,
    time: NaiveDateTime,

    money_earned: f64,
    distance_covered: f64,
    h: f64,
}

impl Node {
    fn new(location: (OrderedFloat<f64>, OrderedFloat<f64>)) -> Node {
        Node {
            location,
            parent: None,
            time: chrono::naive::MAX_DATETIME,

            money_earned: 0.,
            distance_covered: 0.,
            h: f64::MIN,
        }
    }

    fn calculate_heuristic(&self) -> f64 {
        self.h
    }
}

impl Ord for Node {
    fn cmp(&self, other: &Self) -> Ordering {
        self.partial_cmp(&other).unwrap_or_else(|| self.time.cmp(&other.time))
    }
}

impl PartialOrd for Node {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        self.money_earned.partial_cmp(&other.money_earned)
    }
}

impl Eq for Node {}

impl PartialEq for Node {
    fn eq(&self, other: &Self) -> bool {
        self.time == other.time && self.money_earned == other.money_earned
    }
}

#[derive(Debug)]
struct Edge {
    distance: f64,
    amount: i32,
    destination: (OrderedFloat<f64>, OrderedFloat<f64>), // This technically references a Node but I can't find a way to do it safely.
}

fn main() {
    // Load the data we're given.
    let data = load_data("/Users/josh/Documents/Programming/codejam_xi/src/data/123Loadboard_CodeJam_2022_dataset.json").unwrap();
    let input = load_input("/Users/josh/Documents/Programming/codejam_xi/src/data/123Loadboard_CodeJam_2022_input_sample_s300.json").unwrap();

    // Create the semi-graph.
    let mut nodes = HashMap::with_capacity(data.len());
    let mut neighbors = HashMap::with_capacity(data.len());
    for datum in data {
        // Read the origin and destination from the line.
        let origin = (OrderedFloat(datum.origin_latitude), OrderedFloat(datum.origin_longitude));
        let destination = (OrderedFloat(datum.destination_latitude), OrderedFloat(datum.destination_longitude));

        // If the origin isn't already in our node list, add it.
        nodes.entry(origin).or_insert(Node::new(destination));

        // If the destination isn't already in our node list, add it.
        nodes.entry(destination).or_insert(Node::new(origin));

        // Add an edge connecting the origin and destination nodes.
        neighbors.entry(origin).or_insert(Vec::new()).push(Edge { distance: get_geodesic_distance((datum.origin_latitude, datum.origin_longitude), (datum.destination_latitude, datum.destination_longitude)), amount: datum.amount, destination });
    }

    // Loop through all the inputs.
    for request in input {
        let mut start_node = Node::new((OrderedFloat(request.start_latitude), OrderedFloat(request.start_longitude)));
        start_node.time = request.start_time;



        let mut open = BinaryHeap::new();
        let mut closed = Vec::new();

        open.push(start_node);

        while let Some(current_node) = open.pop() {
            // Check all neighbours
            for edge in neighbors.get(&current_node.location).unwrap_or(&vec![]) {
                let edge_cost = (current_node.money_earned + edge.amount as f64) - ((current_node.distance_covered + edge.distance) * FUEL_COST_PER_MILE);
                let neighbor_node = Node {
                    parent: Some(current_node.location),
                    time: current_node.time + chrono::Duration::seconds((edge.distance / MILES_TRAVELLED_PER_HOUR as f64 * 3600.) as i64),
                    money_earned: current_node.money_earned + edge.amount as f64,
                    distance_covered: current_node.distance_covered + edge.distance,
                    h: edge_cost,
                    ..current_node
                };

                if (edge_cost)

                if !closed.contains(&edge.destination) && edge_cost > neighbor_node.h {
                    if neighbor_node.time < request.max_destination_time {
                        open.push(neighbor_node);
                    }
                }
            }

            // // Check all other nodes.
            // for (key, value) in &nodes {
            //     if *value == current_node {
            //         continue;
            //     }
            //     let distance = get_geodesic_distance((current_node.location.0.into_inner(), current_node.location.1.into_inner()), (key.0.into_inner(), key.1.into_inner()));
            //     let edge_cost = current_node.money_earned - ((current_node.distance_covered + distance) * FUEL_COST_PER_MILE);
            //     let next_node = Node {
            //         parent: Some(current_node.location),
            //         time: current_node.time + chrono::Duration::seconds((distance / MILES_TRAVELLED_PER_HOUR as f64 * 3600.) as i64),
            //         distance_covered: current_node.distance_covered + distance,
            //         h: edge_cost,
            //         ..current_node
            //     };
            //     if next_node.time < request.max_destination_time {
            //         open.push(next_node);
            //     }
            // }
        }
    }
}