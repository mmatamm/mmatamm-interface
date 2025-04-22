mod hdf5_fetcher;
mod questdb_fetcher;

use std::{error::Error as StdError, fmt::Display, future::Future};

use chrono::{DateTime, NaiveDateTime, Utc};
pub use hdf5_fetcher::HDF5Fetcher;
pub use questdb_fetcher::QuestDbFetcher;

use crate::market::SystemEvent;

// TODO XXX Return the timestamp of the relevant row

pub trait Fetcher: Display {
    type Error: StdError + Send;

    fn query_price(
        &mut self,
        time: &DateTime<Utc>,
        symbol: &str,
    ) -> impl Future<Output = Result<Option<f64>, Self::Error>> + Send;

    fn query_system_event(
        &mut self,
        time: &DateTime<Utc>,
    ) -> impl Future<Output = Result<Option<(SystemEvent, NaiveDateTime)>, Self::Error>> + Send;
}
