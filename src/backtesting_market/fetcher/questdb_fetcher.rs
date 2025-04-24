use chrono::NaiveDateTime;
use tokio::try_join;
use tokio_postgres::Statement;

use crate::{backtesting_market::query_engine::Ohlc, market::SystemEvent};

use super::Fetcher;

#[derive(Debug)]
pub struct QuestDbFetcher {
    /// A database client
    db_client: tokio_postgres::Client,

    /// A prepared statement for querying all the trade prices of an equity
    prices_query_statement: Statement,
    /// A prepared statement for qureying all the system events
    system_events_query_statement: Statement,
}

impl QuestDbFetcher {
    pub async fn new(db_client: tokio_postgres::Client) -> Result<Self, Error> {
        let (prices_query_statement, system_events_query_statement) = try_join!(
            // TODO no need to query the symbol duh
            db_client.prepare("SELECT * FROM prices WHERE symbol = $1::TEXT;",),
            db_client.prepare("SELECT * FROM system_events;"),
        )?;

        Ok(Self {
            db_client,
            prices_query_statement,
            system_events_query_statement,
        })
    }
}

impl Fetcher for QuestDbFetcher {
    type Error = Error;

    // async fn query_price(
    //     &mut self,
    //     time: &DateTime<Utc>,
    //     symbol: &str,
    // ) -> Result<Option<f32>, Error> {
    //     let row_opt = self
    //         .db_client
    //         .query_opt(
    //             &self.price_query_statement,
    //             &[&(time.timestamp_micros() as f64), &symbol],
    //         )
    //         .await?;

    //     if let Some(row) = row_opt {
    //         Ok(Some(row.get(4)))
    //     } else {
    //         Ok(None)
    //     }
    // }

    // async fn query_system_event(
    //     &mut self,
    //     time: &DateTime<Utc>,
    // ) -> Result<Option<(SystemEvent, NaiveDateTime)>, Error> {
    //     let row_opt = self
    //         .db_client
    //         .query_opt(
    //             &self.system_event_query_statement,
    //             &[&(time.timestamp_micros() as f64)],
    //         )
    //         .await?;

    //     if let Some(row) = row_opt {
    //         let event_type_str: String = row.get(0);

    //         let event_type = match event_type_str.as_str() {
    //             "system_hours_start" => Ok(SystemEvent::PreMarketStart),
    //             "regular_hours_start" => Ok(SystemEvent::RegularMarketStart),
    //             "regular_hours_end" => Ok(SystemEvent::RegularMarketEnd),
    //             "system_hours_end" => Ok(SystemEvent::PostMarketEnd),
    //             symbol => Err(Error::UnexpectedDatabaseSymbol {
    //                 symbol: symbol.to_string(),
    //                 expected_kind: "system event".to_string(),
    //             }),
    //         }?;

    //         Ok(Some((event_type, row.get(1))))
    //     } else {
    //         Ok(None)
    //     }
    // }

    // TODO I want to be able to query only a range
    async fn fetch_system_events(&self) -> Result<Vec<(i64, SystemEvent)>, Self::Error> {
        let rows = self
            .db_client
            .query(&self.system_events_query_statement, &[])
            .await?;

        rows.iter()
            .map(|row| {
                let event_type_str: String = row.get(0);

                let event_type = match event_type_str.as_str() {
                    "system_hours_start" => Ok(SystemEvent::PreMarketStart),
                    "regular_hours_start" => Ok(SystemEvent::RegularMarketStart),
                    "regular_hours_end" => Ok(SystemEvent::RegularMarketEnd),
                    "system_hours_end" => Ok(SystemEvent::PostMarketEnd),
                    symbol => Err(Error::UnexpectedDatabaseSymbol {
                        symbol: symbol.to_string(),
                        expected_kind: "system event".to_string(),
                    }),
                }?;

                let time: NaiveDateTime = row.get(1);
                let timestamp = time.and_utc().timestamp();
                // let timestamp: i64 = row.get(1);
                Ok((timestamp, event_type))
            })
            .collect()
    }

    async fn fetch_ticker_prices(
        &self,
        symbol: &str,
    ) -> Result<Vec<(i64, crate::backtesting_market::query_engine::Ohlc)>, Self::Error> {
        let rows = self
            .db_client
            .query(&self.prices_query_statement, &[&symbol])
            .await?;

        rows.iter()
            .map(|row| {
                // let timestamp: i64 = row.get(5);
                let time: NaiveDateTime = row.get(5);
                let timestamp = time.and_utc().timestamp();

                let open: f64 = row.get(1);
                let high: f64 = row.get(2);
                let low: f64 = row.get(3);
                let close: f64 = row.get(4);

                let ohlc = Ohlc {
                    open: open as f32,
                    high: high as f32,
                    low: low as f32,
                    close: close as f32,
                };
                Ok((timestamp, ohlc))
            })
            .collect()
        //     let row_opt = self
        //         .db_client
        //         .query_opt(
        //             &self.price_query_statement,
        //             &[&(time.timestamp_micros() as f64), &symbol],
        //         )
        //         .await?;

        //     if let Some(row) = row_opt {
        //         Ok(Some(row.get(4)))
        //     } else {
        //         Ok(None)
        //     }
    }
}

#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error("PostgreSQL error")]
    DatabaseError(#[from] tokio_postgres::Error),

    #[error(
        "Symbol '{symbol}' found in database, which is not of the expected kind, {expected_kind}"
    )]
    UnexpectedDatabaseSymbol {
        symbol: String,
        expected_kind: String,
    },
}

impl std::fmt::Display for QuestDbFetcher {
    fn fmt(&self, _f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        todo!()
    }
}
