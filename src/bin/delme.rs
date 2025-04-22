use std::{collections::VecDeque, error::Error};

use chrono::{DateTime, DurationRound, TimeDelta, Utc};
use mmatamm_interface::{
    backtesting_market::{fetcher::HDF5Fetcher, BacktestingMarket},
    market::{Event, Market, MarketTime, SystemEvent},
    Algorithm,
};
use tokio::sync::RwLock;

struct CrossMovingAverageStrategy {
    symbol: String,
    timestep_duration: TimeDelta,
    short_ma_duration: usize,
    long_ma_duration: usize,

    short_ma_samples: VecDeque<f64>,
    long_ma_samples: VecDeque<f64>,

    last_bought: bool,
    last_sold: bool,
}

impl CrossMovingAverageStrategy {
    pub fn new(
        symbol: &str,
        timestep_duration: TimeDelta,
        short_ma_duration: usize,
        long_ma_duration: usize,
    ) -> Self {
        assert!(long_ma_duration > short_ma_duration);

        CrossMovingAverageStrategy {
            symbol: symbol.to_string(),
            timestep_duration,
            short_ma_duration,
            long_ma_duration,

            short_ma_samples: VecDeque::new(),
            long_ma_samples: VecDeque::new(),

            last_bought: false,
            last_sold: false,
        }
    }
}

impl Algorithm for CrossMovingAverageStrategy {
    fn wake_ups() -> impl Iterator<Item = chrono::NaiveTime> {
        vec![].into_iter()
    }

    async fn run<M: Market>(&mut self, market: &mut M) -> Result<(), M::Error> {
        // Wait for the market to initialy open
        loop {
            let ev = market.next_event().await?.expect("No events").1;
            if ev == Event::SystemEvent(SystemEvent::RegularMarketStart) {
                break;
            }
        }
        // assert_eq!(Event::SystemEvent(SystemEvent::RegularMarketStart));

        let mut next_tick = market
            .time()
            .duration_trunc(self.timestep_duration)
            .unwrap()
            + self.timestep_duration;

        for _ in 0..3000 {
            let (_, event) = market.next_event_until(next_tick).await?;
            if event != Event::Deadline {
                continue;
            }

            next_tick += self.timestep_duration;

            if market.market_time() != MarketTime::Regular {
                continue;
            }

            let current_price = market.current_price(&self.symbol).await?;
            self.long_ma_samples.push_front(current_price);
            self.short_ma_samples.push_front(current_price);

            if self.long_ma_samples.len() > self.long_ma_duration {
                let _ = self.long_ma_samples.pop_back().unwrap();
            }
            if self.short_ma_samples.len() > self.short_ma_duration {
                let _ = self.short_ma_samples.pop_back().unwrap();
            }

            if self.long_ma_samples.len() == self.long_ma_duration {
                let long_ma_sum: f64 = self.long_ma_samples.iter().sum();
                let short_ma_sum: f64 = self.short_ma_samples.iter().sum();
                let long_ma = long_ma_sum / self.long_ma_duration as f64;
                let short_ma = short_ma_sum / self.short_ma_duration as f64;

                if short_ma > long_ma {
                    if !self.last_bought {
                        // buy
                        // TODO add a market extender function for this
                        let quantity = market.cash() / current_price;
                        market.buy_at_market(&self.symbol, quantity as u32).await?;
                        println!("buying {} shares", quantity as u32);

                        self.last_bought = true;
                        self.last_sold = false;
                    }
                } else if !self.last_sold {
                    // sell
                    // TODO add a market extender function for this
                    let quantity = market.shares_of(&self.symbol);
                    market.sell_at_market(&self.symbol, quantity).await?;
                    println!("selling {} shares", quantity);

                    self.last_bought = false;
                    self.last_sold = true;
                }
            }
        }

        println!(
            "net worth: {}",
            market.cash()
                + (market.shares_of(&self.symbol) as f64)
                    * market.current_price(&self.symbol).await?
        );

        Ok(())
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    flexi_logger::Logger::try_with_env()
        .unwrap()
        .start()
        .unwrap();

    let fetcher = RwLock::new(HDF5Fetcher::new(
        "../mmatamm-fetch/scripts/mmatamm-qdb2hdf5/output.h5",
    )?);

    let mut market = BacktestingMarket::new(
        &fetcher,
        "2024-06-25T13:00:00Z".parse::<DateTime<Utc>>()?,
        10_000.0,
    )
    .await?;
    let mut myalgo = CrossMovingAverageStrategy::new("PLTR", TimeDelta::minutes(5), 5, 10);
    myalgo.run(&mut market).await?;

    Ok(())
}
