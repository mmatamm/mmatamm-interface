use std::path::Path;

use hdf5;
use ndarray::{s, Array2, Axis};
use thiserror;

use crate::{backtesting_market::query_engine::Ohlc, market::SystemEvent};

use super::Fetcher;

#[derive(Debug)]
pub struct HDF5Fetcher {
    file: hdf5::File,
    prices_group: hdf5::Group,
}

// TODO is this necessary?
impl std::fmt::Display for HDF5Fetcher {
    fn fmt(&self, _f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        todo!()
    }
}

impl HDF5Fetcher {
    pub fn new<P: AsRef<Path>>(filename: P) -> Result<Self, Error> {
        let file = hdf5::File::open(filename)?;
        let prices_group = file.group("prices")?;

        Ok(Self { file, prices_group })
    }
}

impl Fetcher for HDF5Fetcher {
    type Error = Error;

    async fn fetch_system_events(&self) -> Result<Vec<(i64, SystemEvent)>, Self::Error> {
        let system_events_array = self
            .file
            .dataset("system_events")?
            .read_slice_2d(s![.., ..])?;
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

        Ok(system_events.to_vec())
    }

    async fn fetch_ticker_prices(&self, symbol: &str) -> Result<Vec<(i64, Ohlc)>, Self::Error> {
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

        Ok(p.to_vec())
    }
}

#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error("HDF5 error")]
    HDF5Error(#[from] hdf5::Error),
}
