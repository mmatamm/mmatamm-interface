mod timestep_series;

use std::path::Path;

use ahash::{HashMap, HashMapExt};
use chrono::{DateTime, NaiveDateTime};
use hdf5;
use ndarray::{s, Array2, Axis};
use thiserror;
use timestep_series::TimestepSeries;

use crate::market::SystemEvent;

use super::Fetcher;

#[derive(Clone, Debug)]
struct Ohlc {
    open: f32,
    high: f32,
    low: f32,
    close: f32,
}

#[derive(Debug)]
pub struct HDF5Fetcher {
    _file: hdf5::File,
    prices_group: hdf5::Group,
    prices_series: HashMap<String, TimestepSeries<Ohlc>>,
    system_events_series: TimestepSeries<SystemEvent>,
}

impl std::fmt::Display for HDF5Fetcher {
    fn fmt(&self, _f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        todo!()
    }
}

impl HDF5Fetcher {
    pub fn new<P: AsRef<Path>>(filename: P) -> Result<Self, Error> {
        let file = hdf5::File::open(filename)?;
        let prices_group = file.group("prices")?;

        let system_events_array = file.dataset("system_events")?.read_slice_2d(s![.., ..])?;
        let system_events = system_events_array.map_axis(Axis(1), |row| {
            let ev = match row[1] {
                0 => SystemEvent::PreMarketStart,
                1 => SystemEvent::RegularMarketStart,
                2 => SystemEvent::RegularMarketEnd,
                3 => SystemEvent::PostMarketEnd,
                _ => panic!("unknown system event {}", row[1]),
            };
            let time = row[0] * 60 as i64;
            (time, ev)
        });
        let system_events_series = TimestepSeries::new(system_events.to_vec());

        Ok(Self {
            _file: file,
            prices_group,
            prices_series: HashMap::new(),
            system_events_series,
        })
    }
}

impl Fetcher for HDF5Fetcher {
    type Error = Error;

    async fn query_price(
        &mut self,
        time: &chrono::DateTime<chrono::Utc>,
        symbol: &str,
    ) -> Result<Option<f32>, Self::Error> {
        if !self.prices_series.contains_key(symbol) {
            let dataset = self.prices_group.dataset(symbol)?;
            let array: Array2<i32> = dataset.read()?;
            let p = array.map_axis(Axis(1), |row| {
                let ohlc = Ohlc {
                    open: row[1] as f32 / 100.0,
                    high: row[2] as f32 / 100.0,
                    low: row[3] as f32 / 100.0,
                    close: row[4] as f32 / 100.0,
                };
                let time = row[0] as i64 * 60;
                (time, ohlc)
            });

            let s = TimestepSeries::new(p.to_vec());
            self.prices_series.insert(symbol.to_owned(), s);
        }

        let series = self.prices_series.get(symbol).unwrap();

        Ok(series
            .query_before(&time.naive_utc())
            .map(|(_time, ohlc)| ohlc.close))
    }

    async fn query_system_event(
        &mut self,
        time: &chrono::DateTime<chrono::Utc>,
    ) -> Result<Option<(crate::market::SystemEvent, chrono::NaiveDateTime)>, Self::Error> {
        // TODO use NaiveDateTime
        let event_opt = self.system_events_series.query_after(&time.naive_utc());
        Ok(event_opt.map(|(timestamp, event)| {
            (
                event.clone(),
                DateTime::from_timestamp(*timestamp, 0).unwrap().naive_utc(),
            )
        }))
    }
}

#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error("HDF5 error")]
    HDF5Error(#[from] hdf5::Error),
}
