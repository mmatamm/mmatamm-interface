use std::collections::{HashMap, LinkedList};

use chrono::{DateTime, DurationRound as _, NaiveDateTime, Utc};
use thiserror::Error;
use tokio::try_join;
use tokio_postgres::Statement;

use crate::market::{Event, ImpossibleEvent, Market, MarketTime};

pub struct QuestDbMarket<'a> {
    /// A database client
    db_client: &'a tokio_postgres::Client,

    /// The current virtual time
    time: DateTime<Utc>,
    /// The current market time (e.g. pre-market, regular hours, etc...)
    market_time: MarketTime,
    /// All the following events. This does not include system events and ticks.
    events: LinkedList<(DateTime<Utc>, Event)>,

    // TODO seperate `cash` to `available_cash` and `locked_cash` (or some other name). =
    // available_cash will be subtracted from when submitting an order, and added to
    // locked_cash. Upon trade complete, this will be updated.
    /// The amount of cash on hand
    cash: f64,
    /// How many shares of each equity are owned, by symbol
    holdings: HashMap<String, u32>,

    /// A prepared statement for querying the N most recent trade prices
    /// of an equity
    price_query_statement: Statement,
    /// A prepared statement for qureying the next system event
    system_event_query_statement: Statement,
}

#[derive(Error, Debug)]
pub enum Error {
    #[error("PostgreSQL error")]
    DatabaseError(#[from] tokio_postgres::Error),

    #[error("Attempted to trade {0} at {1}, outside of trading hours")]
    UntimelyTrade(String, DateTime<Utc>),

    #[error("Attempted to trade {0} yet the price is unknown")]
    UnknownPrice(String),

    #[error("Cannot buy {quantity} shares of {symbol} for {total_price} with {cash} in cash")]
    InsufficientCash {
        quantity: u32,
        symbol: String,
        total_price: f64,
        cash: f64,
    },

    #[error("Cannot sell {quantity} shares of {symbol} because only {owned} shares are owned")]
    InsufficientShares {
        quantity: u32,
        symbol: String,
        owned: u32,
    },

    #[error(
        "Symbol '{symbol}' found in database, which is not of the expected kind, {expected_kind}"
    )]
    UnexpectedDatabaseSymbol {
        symbol: String,
        expected_kind: String,
    },

    #[error("Impossible event, internal logic fault")]
    ImpossibleEvent(#[from] ImpossibleEvent),

    #[error("Tried to query data from {future_time} at {current_time}")]
    FutureQuery {
        future_time: DateTime<Utc>,
        current_time: DateTime<Utc>,
    },
}

impl<'a> QuestDbMarket<'a> {
    pub async fn new(
        database: &'a tokio_postgres::Client,
        start: DateTime<Utc>,
        cash: f64,
    ) -> Result<Self, Error> {
        let (price_query_statement, system_event_query_statement) = try_join!(
            database.prepare(
                "SELECT * FROM ticks WHERE timestamp <= $1::TIMESTAMP AND symbol = $2::TEXT ORDER BY timestamp DESC LIMIT $3::INT;",
            ),
            database.prepare(
                "SELECT * FROM system_events WHERE timestamp > $1::TIMESTAMP ORDER BY timestamp ASC LIMIT 1;"
            ),
        )?;

        Ok(QuestDbMarket {
            db_client: database,

            time: start,
            market_time: MarketTime::Unknown,
            events: LinkedList::new(),

            cash,
            holdings: HashMap::new(),

            price_query_statement,
            system_event_query_statement,
        })
    }

    async fn next_system_event(&self) -> Result<Option<(DateTime<Utc>, Event)>, Error> {
        if let Some(next_row) = self
            .db_client
            .query_opt(
                &self.system_event_query_statement,
                &[&(self.time.timestamp_micros() as f64)],
            )
            .await?
        {
            let event_type = match next_row.get(0) {
                "system_hours_start" => Ok(Event::PreMarketStart),
                "regular_hours_start" => Ok(Event::RegularMarketStart),
                "regular_hours_end" => Ok(Event::RegularMarketEnd),
                "system_hours_end" => Ok(Event::PostMarketEnd),
                symbol => Err(Error::UnexpectedDatabaseSymbol {
                    symbol: symbol.to_string(),
                    expected_kind: "system event".to_string(),
                }),
            }?;

            let timestamp: NaiveDateTime = next_row.get(1);
            // let timestamp = DateTime::from_sql(Timestamp, next_row.get(1));

            Ok(Some((timestamp.and_utc(), event_type)))
        } else {
            Ok(None)
        }
    }

    async fn peek_next_event(&self) -> Result<Option<(DateTime<Utc>, Event)>, Error> {
        let next_system_event = self.next_system_event().await?;
        let next_internal_event = self.events.front();

        match (next_system_event, next_internal_event) {
            (Some(next_sys), Some(next_int)) => {
                if next_sys.0 >= next_int.0 {
                    Ok(Some(next_int.clone()))
                } else {
                    Ok(Some(next_sys))
                }
            }
            (Some(next_sys), None) => Ok(Some(next_sys)),
            (None, Some(next_int)) => Ok(Some(next_int.clone())),
            (None, None) => Ok(None),
        }
    }
}

impl<'a> Market for QuestDbMarket<'a> {
    type Error = Error;

    async fn next_event(&mut self) -> Result<Option<(DateTime<Utc>, Event)>, Error> {
        match self.peek_next_event().await? {
            Some((time, event)) => {
                self.time = time;
                self.market_time.update(&event)?;

                // TODO if the event is internal, pop it from the linked list

                Ok(Some((time, event)))
            }
            None => Ok(None),
        }
    }

    async fn next_event_or_tick(
        &mut self,
        tick: chrono::TimeDelta,
    ) -> Result<(DateTime<Utc>, Event), Error> {
        let next_tick = self.time.duration_trunc(tick).unwrap() + tick;

        let event = if let Some((time, event)) = self.peek_next_event().await? {
            if time <= next_tick {
                self.market_time.update(&event)?;

                // TODO if the event is internal, pop it from the linked list
                (time, event)
            } else {
                (next_tick, Event::Tick)
            }
        } else {
            (next_tick, Event::Tick)
        };

        self.time = event.0;

        Ok(event)
    }

    fn time(&self) -> DateTime<Utc> {
        self.time
    }

    async fn price_at(&self, symbol: &str, time: DateTime<Utc>) -> Result<f64, Error> {
        // TODO Remember the random value for a stock and deviate from it using
        // geometric Brownian motion (or some estimation of it). Assume the
        // price is in the middle of the bid/ask spread
        // TODO Verify the timestamps
        // TODO Implement speculative pre-fetching
        // TODO Avoid querying future prices
        // TODO Consider introducing a 15-minutes delay

        if time > self.time {
            return Err(Error::FutureQuery {
                future_time: time,
                current_time: self.time,
            });
        }

        let row = self
            .db_client
            .query_opt(
                &self.price_query_statement,
                &[&(time.timestamp_micros() as f64), &symbol, &1f64],
            )
            .await?
            .ok_or(Error::UnknownPrice(symbol.to_string()))?;

        // Return the last close price
        Ok(row.get(4))
    }

    async fn buy_at_market(&mut self, symbol: &str, quantity: u32) -> Result<(), Error> {
        // Ensure the market is open
        if !self.market_time.is_open() {
            return Err(Error::UntimelyTrade(symbol.to_string(), self.time));
        }

        // Calculate the transaction's cost
        // TODO include fees, bid and ask too
        let price_per_share = self.current_price(symbol).await?;
        let total_price = price_per_share * quantity as f64;

        // Ensure the cash is sufficient for it
        if total_price > self.cash {
            return Err(Error::InsufficientCash {
                quantity,
                symbol: symbol.to_string(),
                total_price,
                cash: self.cash,
            });
        }

        // Update the cash and the holdings
        self.cash -= total_price;

        if let Some(v) = self.holdings.get_mut(symbol) {
            *v += quantity;
        } else {
            self.holdings.insert(symbol.to_string(), quantity);
        }

        // TODO Add an event of PurchaseComplete
        // TODO The transaction might be canceled if it's at the end of the
        // day and there are no buyers/sellers

        Ok(())
    }

    async fn sell_at_market(&mut self, symbol: &str, quantity: u32) -> Result<(), Error> {
        // Ensure the market is open
        if !self.market_time.is_open() {
            return Err(Error::UntimelyTrade(symbol.to_string(), self.time));
        }

        // Calculate the transaction's cost
        // TODO include fees, bid and ask too
        let price_per_share = self.current_price(symbol).await?;
        let total_price = price_per_share * quantity as f64;

        // Ensure there are enough shares of this stock
        let owned_shares_opt = self.holdings.get_mut(symbol);
        if owned_shares_opt.is_none() {
            return Err(Error::InsufficientShares {
                quantity,
                symbol: symbol.to_string(),
                owned: 0,
            });
        }

        if &quantity > owned_shares_opt.as_ref().unwrap() {
            return Err(Error::InsufficientShares {
                quantity,
                symbol: symbol.to_string(),
                owned: *owned_shares_opt.unwrap(),
            });
        }

        // Update the cash and the holdings
        self.cash += total_price;

        if let Some(v) = self.holdings.get_mut(symbol) {
            *v -= quantity
        } else {
            unreachable!()
        }

        // TODO Add an event of SellComplete
        // TODO The transaction might be canceled if it's at the end of the
        // day and there are no buyers/sellers

        Ok(())
    }

    fn market_time(&self) -> crate::market::MarketTime {
        self.market_time
    }

    fn cash(&self) -> f64 {
        self.cash
    }

    fn shares_of(&self, symbol: &str) -> u32 {
        if let Some(q) = self.holdings.get(symbol) {
            *q
        } else {
            0
        }
    }

    fn holdings(&self) -> impl IntoIterator<Item = (&String, &u32)> {
        &self.holdings
    }
}
