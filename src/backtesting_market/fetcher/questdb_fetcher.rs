use chrono::{DateTime, NaiveDateTime, Utc};
use tokio::try_join;
use tokio_postgres::Statement;

use crate::market::SystemEvent;

use super::Fetcher;

#[derive(Debug)]
pub struct QuestDbFetcher {
    /// A database client
    db_client: tokio_postgres::Client,

    /// A prepared statement for querying the N most recent trade prices
    /// of an equity
    price_query_statement: Statement,
    /// A prepared statement for qureying the next system event
    system_event_query_statement: Statement,
}

impl QuestDbFetcher {
    pub async fn new(db_client: tokio_postgres::Client) -> Result<Self, Error> {
        let (price_query_statement, system_event_query_statement) = try_join!(
            db_client.prepare(
                "SELECT * FROM prices WHERE timestamp <= $1::TIMESTAMP AND symbol = $2::TEXT ORDER BY timestamp DESC LIMIT 1;",
            ),
            db_client.prepare(
                "SELECT * FROM system_events WHERE timestamp > $1::TIMESTAMP ORDER BY timestamp ASC LIMIT 1;"
            ),
        )?;

        Ok(Self {
            db_client,
            price_query_statement,
            system_event_query_statement,
        })
    }
}

impl Fetcher for QuestDbFetcher {
    type Error = Error;

    async fn query_price(
        &mut self,
        time: &DateTime<Utc>,
        symbol: &str,
    ) -> Result<Option<f32>, Error> {
        let row_opt = self
            .db_client
            .query_opt(
                &self.price_query_statement,
                &[&(time.timestamp_micros() as f64), &symbol],
            )
            .await?;

        if let Some(row) = row_opt {
            Ok(Some(row.get(4)))
        } else {
            Ok(None)
        }
    }

    async fn query_system_event(
        &mut self,
        time: &DateTime<Utc>,
    ) -> Result<Option<(SystemEvent, NaiveDateTime)>, Error> {
        let row_opt = self
            .db_client
            .query_opt(
                &self.system_event_query_statement,
                &[&(time.timestamp_micros() as f64)],
            )
            .await?;

        if let Some(row) = row_opt {
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

            Ok(Some((event_type, row.get(1))))
        } else {
            Ok(None)
        }
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
