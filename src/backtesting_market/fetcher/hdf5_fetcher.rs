mod timestep_series;

use std::path::Path;

use ahash::{HashMap, HashMapExt};
use hdf5;
use thiserror;
use timestep_series::TimestepSeries;

use crate::market::SystemEvent;

use super::Fetcher;

#[derive(Debug)]
pub struct HDF5Fetcher {
    _file: hdf5::File,
    prices_group: hdf5::Group,
    prices_series: HashMap<String, TimestepSeries>,
    system_events_dataset: TimestepSeries,
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
        let system_events_dataset = TimestepSeries::new(file.dataset("system_events")?, 60.0)?;

        Ok(Self {
            _file: file,
            prices_group,
            prices_series: HashMap::new(),
            system_events_dataset,
        })
    }
}

impl Fetcher for HDF5Fetcher {
    type Error = Error;

    async fn query_price(
        &mut self,
        time: &chrono::DateTime<chrono::Utc>,
        symbol: &str,
    ) -> Result<Option<f64>, Self::Error> {
        if !self.prices_series.contains_key(symbol) {
            let ds = self.prices_group.dataset(symbol)?;
            let s = TimestepSeries::new(ds, 60.0)?;
            self.prices_series.insert(symbol.to_owned(), s);
        }

        let series = self.prices_series.get(symbol).unwrap();

        let row_opt = series.query_before(&time.naive_utc())?;
        // let row = series.query_uniform(&time.naive_utc(), false, 60.0)?;

        Ok(row_opt.map(|row| (row[4] as f64) / 100.0))
    }

    async fn query_system_event(
        &mut self,
        time: &chrono::DateTime<chrono::Utc>,
    ) -> Result<Option<(crate::market::SystemEvent, chrono::NaiveDateTime)>, Self::Error> {
        // TODO use NaiveDateTime
        let row_opt = self.system_events_dataset.query_after(&time.naive_utc())?;

        Ok(row_opt.map(|row| {
            let actual_ts = row[0] as i64;
            // This simple `from_timestamp` takes about 4.8% of the total samples!
            let actual_dt = chrono::DateTime::from_timestamp(actual_ts * 60, 0).unwrap();

            let ev = match row[1] {
                0 => SystemEvent::PreMarketStart,
                1 => SystemEvent::RegularMarketStart,
                2 => SystemEvent::RegularMarketEnd,
                3 => SystemEvent::PostMarketEnd,
                _ => panic!("unknown system event"),
            };

            (ev, actual_dt.naive_utc())
        }))
    }
}

#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error("HDF5 error")]
    HDF5Error(#[from] hdf5::Error),
}
