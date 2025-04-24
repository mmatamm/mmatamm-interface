mod timestep_series;

use ahash::{HashMap, HashMapExt};
use chrono::DateTime;
use timestep_series::TimestepSeries;

use crate::market::SystemEvent;

use super::Fetcher;

#[derive(Clone, Debug)]
pub struct Ohlc {
    // TODO PERF I don't need the open price. Maybe I don't even need the close price.
    // TODO PERF Maybe storing the rest of the values relatively to open is a good idea,
    // computation is cheaper than memory...
    pub open: f32,
    pub high: f32,
    pub low: f32,
    pub close: f32,
}

#[derive(Debug)]
pub struct QueryEngine<F: Fetcher> {
    fetcher: F,

    // TODO PERF Use DashMap and make this thing Sync/Send
    prices_series: HashMap<String, TimestepSeries<Ohlc>>,
    system_events_series: TimestepSeries<SystemEvent>,
}

// impl std::fmt::Display for QueryEngine {
//     fn fmt(&self, _f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
//         todo!()
//     }
// }

impl<F: Fetcher> QueryEngine<F> {
    pub async fn new(fetcher: F) -> Result<Self, F::Error> {
        let system_events_series = TimestepSeries::new(fetcher.fetch_system_events().await?);

        Ok(Self {
            fetcher,
            prices_series: HashMap::new(),
            system_events_series,
        })
    }

    pub async fn query_price(
        &mut self,
        time: &chrono::DateTime<chrono::Utc>,
        symbol: &str,
    ) -> Result<Option<f32>, F::Error> {
        if !self.prices_series.contains_key(symbol) {
            let s = TimestepSeries::new(self.fetcher.fetch_ticker_prices(symbol).await?);
            self.prices_series.insert(symbol.to_owned(), s);
        }

        let series = self.prices_series.get(symbol).unwrap();

        Ok(series
            .query_before(&time.naive_utc())
            .map(|(_time, ohlc)| ohlc.close))
    }

    pub async fn query_system_event(
        &mut self,
        time: &chrono::DateTime<chrono::Utc>,
    ) -> Option<(crate::market::SystemEvent, chrono::NaiveDateTime)> {
        // TODO use NaiveDateTime
        let event_opt = self.system_events_series.query_after(&time.naive_utc());
        event_opt.map(|(timestamp, event)| {
            (
                event.clone(),
                DateTime::from_timestamp(*timestamp, 0).unwrap().naive_utc(),
            )
        })
    }
}
