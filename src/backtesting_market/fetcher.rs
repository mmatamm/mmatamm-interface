mod hdf5_fetcher;
mod questdb_fetcher;

use std::{error::Error as StdError, fmt::Display, future::Future};

use super::query_engine::Ohlc;
pub use hdf5_fetcher::HDF5Fetcher;
pub use questdb_fetcher::QuestDbFetcher;

use crate::market::SystemEvent;

// TODO XXX Return the timestamp of the relevant row
// TODO The `: Display` shouldn't be here

pub trait Fetcher: Display {
    type Error: StdError + Send;

    fn fetch_system_events(
        &self,
    ) -> impl Future<Output = Result<Vec<(i64, SystemEvent)>, Self::Error>> + Send;
    fn fetch_ticker_prices(
        &self,
        symbol: &str,
    ) -> impl Future<Output = Result<Vec<(i64, Ohlc)>, Self::Error>> + Send;
}
