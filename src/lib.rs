#![cfg_attr(feature = "serde_derive", feature(proc_macro))]

#[cfg(feature = "serde_derive")]
#[macro_use]
extern crate serde_derive;
extern crate serde;
extern crate serde_json;
extern crate csv;
extern crate geo;
extern crate rustc_serialize;
extern crate rayon;

#[cfg(feature = "serde_derive")]
include!("geojson.in.rs");

#[cfg(feature = "serde_codegen")]
include!(concat!(env!("OUT_DIR"), "/geojson.rs"));

pub mod error;

use rayon::prelude::*;
use geo::{Point, Polygon, LineString};
use geo::algorithm::contains::Contains;
use std::result;
use std::fs::File;
use std::io::prelude::*;

pub type Result<T> = result::Result<T, error::Error>;

#[derive(Debug, RustcDecodable)]
pub struct Record {
    pub index: i32,
    pub testid: i64,
    pub longitude: f64,
    pub latitude: f64,
}

impl Record {
    /// Returns a point with latitude and longitude (in that order).
    pub fn position(&self) -> geo::Point<f64> {
        Point::new(self.latitude, self.longitude)
    }
}

#[test]
fn test_record() {
    let record = Record { index: 0, testid: 1000, longitude: 59.210860, latitude: 8.009823 };
    let point  = Point::new(8.009823, 59.210860);
    assert_eq!(record.position(), point);
}

#[derive(Debug)]
pub struct County {
    name: String,
    poly: geo::Polygon<f64>,
}

impl County {
    /// Create a new County object from a Feature.
    pub fn new(feat: &Feature) -> County {
        let mut points = vec![];

        // Extract the first/only array of coordinates (external borders)
        let coords = feat.geometry.coordinates[0].clone();
        for coord in coords {
            let p = Point::new(coord[1].clone(), coord[0].clone());
            points.push(p);
        }

        County {
            name: feat.properties.navn.clone(),
            poly: Polygon::new(LineString(points), vec![])
        }
    }
    /// Checks whether a point is in a county.
    /// Returns the name of the county.
    pub fn lookup(&self, p: &geo::Point<f64>) -> Option<String> {
        match self.poly.contains(p) {
            true  => Some(self.name.clone()),
            false => None,
        }
    }

    /// Lookup a record and return the testid and county if any.
    /// Returns a tuple with the testid and name of the county.
    pub fn lookup_record(&self, p: &Record) -> Option<(i64, String)> {
        match self.poly.contains(&p.position()) {
            true  => Some((p.testid, self.name.clone())),
            false => None,
        }
    }
}

#[test]
fn test_county() {
    let json = read_geojson("./examples/data/sample.geojson").unwrap();
    let res  = County::new(&json.features[0]);
    assert_eq!(res.name, "Osterøy");
    assert_eq!(res.lookup(&Point::new(60.524035, 5.552604)).unwrap(), "Osterøy");
}

#[derive(Debug)]
pub struct Counties {
    list: Vec<County>,
}

impl Counties {
    /// Create a new Counties object from a GeoJson.
    pub fn new(json: &GeoJson) -> Counties {
        let mut counties: Vec<County> = vec![];
        for county in json.features.iter() {
            counties.push(County::new(county));
        }
        Counties {
            list: counties
        }
    }
    /// Lookup the county (if any) for a given point.
    /// Returns the name of the county.
    pub fn lookup(&self, p: &geo::Point<f64>) -> Option<String> {
        for kommune in self.list.iter() {
            match kommune.lookup(p) {
                Some(v) => {
                    return Some(v);
                },
                None    => {},
            }
        }
        // No county -> None
        None
    }

    /// Lookup the county (if any) for a record.
    /// Returns a tuple with the name of the county and testid.
    pub fn lookup_record(&self, p: &Record) -> Option<(i64, String)> {
        for kommune in self.list.iter() {
            match kommune.lookup_record(p) {
                Some(v) => {
                    return Some(v);
                },
                None    => {},
            }
        }
        // No county -> None
        None
    }

    /// Lookup multiple locations in parallel.
    pub fn lookup_all(&self, p: &Vec<geo::Point<f64>>) -> Vec<Option<String>> {
        let mut res = Vec::with_capacity(p.len());
        p.par_iter().map(|&point| self.lookup(&point)).collect_into(&mut res);
        res
    }

    /// Lookup multiple records in parallel.
    pub fn lookup_all_records(&self, p: &Vec<Record>) -> Vec<Option<(i64, String)>> {
        let mut res = Vec::with_capacity(p.len());
        p.par_iter().map(|rec| self.lookup_record(&rec)).collect_into(&mut res);
        res
    }
}

#[test]
fn test_counties() {
    let json = read_geojson("./examples/data/sample.geojson").unwrap();
    let res  = Counties::new(&json);
    let p    = Point::new(60.524035, 5.552604);
    let v    = vec![p, p];
    assert_eq!(res.lookup(&p).unwrap(), "Osterøy");
    assert_eq!(res.lookup_all(&v)[0], Some("Osterøy".to_string()));
}

/// Read the 'kommuner.geojson' file. The structure is predefined and should
/// not be changed.
///
/// # Arguments
/// `file`: A borrowed string with the path to the JSON to be read.
///
/// # Returns
/// A `Result` a GeoJson struct.
///
pub fn read_geojson(file: &str) -> Result<GeoJson> {
    let mut f = try!(File::open(file));
    let mut s = String::new();
    try!(f.read_to_string(&mut s));
    let res: GeoJson = try!(serde_json::from_str(&s));
    Ok(res)
}

#[test]
fn test_read_geojson() {
    let res = read_geojson("./examples/data/sample.geojson");
    match res {
        Ok(v)    => {
            assert_eq!(v.kind, "FeatureCollection");
            assert_eq!(v.features[0].kind, "Feature");
        },
        Err(err) => panic!("Error: {:?}", err),
    }
}


/// Read a test records containing index, testid, longitude and latitude.
///
/// # Arguments
/// `file`: A borrowed string with the path to the CSV file to be read.
///
/// # Returns
/// A `Result` with a vector of records, where each record is a line in the CSV.
///
pub fn read_csv(file: &str) -> Result<Vec<Record>> {
    let mut csv = try!(csv::Reader::from_file(&file));
    let mut res: Vec<Record> = vec![];
    for line in csv.decode() {
        let record: Record = try!(line);
        res.push(record);
    }
    Ok(res)
}

#[test]
fn test_read_csv() {
    let res = read_csv("./examples/data/sample.csv");
    match res {
        Ok(v)    => {
            // TODO: This is the 2nd row. Why?
            assert_eq!(v[0].testid, 2200000002);
            assert_eq!(v[0].longitude, 11.0531);
            assert_eq!(v[0].latitude, 59.2761);
        },
        Err(err) => panic!("Error: {:?}", err),
    }
}

