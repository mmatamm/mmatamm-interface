use core::panic;
use std::{
    collections::{HashMap, VecDeque},
    ops::Range,
};

use chrono::{DateTime, DurationRound, RoundingError, TimeDelta, TimeZone, Utc};
use float_eq::{assert_float_eq, float_eq};
use rand::Rng;
use tokio;

use crate::market::{Event, Market, MarketTime};

pub struct TestMarket {
    events: VecDeque<(DateTime<Utc>, Event)>,
    time: DateTime<Utc>,
    next_time: DateTime<Utc>,
    market_time: MarketTime,

    price_histories: HashMap<String, Vec<Range<f64>>>,
    price_history_start: DateTime<Utc>,
    price_history_interval: TimeDelta,

    cash: f64,
    holdings: HashMap<String, u32>,
}

impl Market for TestMarket {
    async fn next_event(&mut self) -> Option<(DateTime<Utc>, Event)> {
        let event = self.events.pop_front();

        if let Some((time, ref event_type)) = event {
            self.market_time.update(event_type);
            self.next_time = time;
            self.time = time;
        }

        event
    }

    async fn next_event_or_tick(
        &mut self,
        tick: chrono::TimeDelta,
    ) -> Result<Option<(DateTime<Utc>, Event)>, RoundingError> {
        let current_tick = self.next_time.duration_trunc(tick)?;
        let next_tick = current_tick + tick;

        if self.next_time == current_tick {
            if let Some((event_time, event)) = self.events.front() {
                if event_time == &self.next_time {
                    self.market_time.update(event);
                    self.time = *event_time;
                    return Ok(self.events.pop_front());
                }
            }

            self.next_time = next_tick;
            self.time = current_tick;
            return Ok(Some((current_tick, Event::Tick)));
        }

        if let Some((event_time, event)) = self.events.front() {
            if event_time <= &next_tick {
                self.market_time.update(event);
                self.next_time = *event_time;
                self.time = *event_time;
                return Ok(self.events.pop_front());
            }
        }

        self.next_time = next_tick;
        self.time = next_tick;
        Ok(Some((next_tick, Event::Tick)))
    }

    fn time(&self) -> DateTime<Utc> {
        self.time
    }

    fn price_at(&self, symbol: &str, time: DateTime<Utc>) -> Option<f64> {
        if time > self.time {
            panic!("tried to access a price from the future without the DeLorian")
        }

        let price_history = self
            .price_histories
            .get(symbol)
            .expect("symbol does not exist");
        let tick_index = (time - self.price_history_start).num_nanoseconds().unwrap()
            / self.price_history_interval.num_nanoseconds().unwrap();

        // NOTE in the actual implementation, consider returning the latest
        // price instead of `None`
        let current_tick = price_history.get(tick_index as usize)?;

        Some(
            if float_eq!(current_tick.start, current_tick.end, ulps <= 5) {
                current_tick.start
            } else {
                let mut rng = rand::thread_rng();
                rng.gen_range(current_tick.clone())
            },
        )
    }

    fn buy_at_market(&mut self, symbol: &str, quantity: u32) {
        let price_per_share = self.current_price(symbol).unwrap();
        let total_price = price_per_share * quantity as f64;

        if total_price > self.cash {
            panic!(
                "Not enough cash: tried to buy {} shares of {} at {} with {} in cash",
                quantity, symbol, price_per_share, self.cash
            );
        }

        self.cash -= total_price;

        let cool = self.holdings.get_mut(symbol);

        if let Some(v) = cool {
            *v += quantity;
        } else {
            self.holdings.insert(symbol.to_string(), quantity);
        }
    }

    fn sell_at_market(&mut self, symbol: &str, quantity: u32) {
        if &quantity > self.holdings.get(symbol).unwrap() {
            panic!(
                "Not enough shares: tried to sell {} shares of {} whilst holding {} shares",
                quantity,
                symbol,
                self.holdings.get(symbol).unwrap()
            );
        }

        let price_per_share = self.current_price(symbol).unwrap();
        let total_price = price_per_share * quantity as f64;

        self.cash += total_price;

        let cool = self.holdings.get_mut(symbol);

        if let Some(v) = cool {
            *v -= quantity;
        } else {
            unreachable!()
        }
    }

    fn market_time(&self) -> MarketTime {
        self.market_time
    }
}

// TODO write a test for irregular ticks

fn assert_event<E>(
    expected_event: Event,
    expected_time: DateTime<Utc>,
    actual_event: Result<Option<(DateTime<Utc>, Event)>, E>,
) {
    assert!(actual_event.is_ok_and(|o| {
        o.is_some_and(|e| {
            let (time, event) = e;
            time == expected_time && event == expected_event
        })
    }));
}

fn assert_in_range<N: PartialOrd>(minimum: N, maximum: N, actual: N) {
    assert!(actual >= minimum && actual <= maximum)
}

#[tokio::test]
async fn test_ticks() {
    let mut market = TestMarket {
        events: VecDeque::new(),
        time: Utc.with_ymd_and_hms(1970, 1, 1, 0, 0, 0).unwrap(),
        next_time: Utc.with_ymd_and_hms(1970, 1, 1, 0, 0, 0).unwrap(),
        market_time: MarketTime::Regular,

        price_histories: HashMap::new(),
        price_history_start: Utc.with_ymd_and_hms(1970, 1, 1, 0, 0, 0).unwrap(),
        price_history_interval: TimeDelta::minutes(1),

        cash: 0.0,
        holdings: HashMap::new(),
    };

    assert!(market.next_event().await.is_none());

    assert_event(
        Event::Tick,
        Utc.with_ymd_and_hms(1970, 1, 1, 0, 0, 0).unwrap(),
        market.next_event_or_tick(TimeDelta::minutes(1)).await,
    );

    assert_eq!(
        market.time(),
        Utc.with_ymd_and_hms(1970, 1, 1, 0, 0, 0).unwrap()
    );

    assert_event(
        Event::Tick,
        Utc.with_ymd_and_hms(1970, 1, 1, 0, 1, 0).unwrap(),
        market.next_event_or_tick(TimeDelta::minutes(1)).await,
    );

    assert_eq!(
        market.time(),
        Utc.with_ymd_and_hms(1970, 1, 1, 0, 1, 0).unwrap()
    );
}

#[tokio::test]
async fn test_market_hours() {
    let mut market = TestMarket {
        events: [(
            Utc.with_ymd_and_hms(1970, 1, 1, 0, 1, 0).unwrap(),
            Event::RegularMarketEnd,
        )]
        .into(),
        time: Utc.with_ymd_and_hms(1970, 1, 1, 0, 0, 0).unwrap(),
        next_time: Utc.with_ymd_and_hms(1970, 1, 1, 0, 0, 0).unwrap(),
        market_time: MarketTime::Regular,

        price_histories: HashMap::new(),
        price_history_start: Utc.with_ymd_and_hms(1970, 1, 1, 0, 0, 0).unwrap(),
        price_history_interval: TimeDelta::minutes(1),

        cash: 0.0,
        holdings: HashMap::new(),
    };

    assert_event(
        Event::Tick,
        Utc.with_ymd_and_hms(1970, 1, 1, 0, 0, 0).unwrap(),
        market.next_event_or_tick(TimeDelta::minutes(1)).await,
    );

    assert_eq!(MarketTime::Regular, market.market_time);

    assert_event(
        Event::RegularMarketEnd,
        Utc.with_ymd_and_hms(1970, 1, 1, 0, 1, 0).unwrap(),
        market.next_event_or_tick(TimeDelta::minutes(1)).await,
    );

    assert_eq!(MarketTime::PostMarket, market.market_time);

    assert_event(
        Event::Tick,
        Utc.with_ymd_and_hms(1970, 1, 1, 0, 1, 0).unwrap(),
        market.next_event_or_tick(TimeDelta::minutes(1)).await,
    );
}

#[tokio::test]
#[should_panic]
async fn test_invalid_market_hours() {
    let mut market = TestMarket {
        events: [(
            Utc.with_ymd_and_hms(1970, 1, 1, 0, 1, 0).unwrap(),
            Event::RegularMarketEnd,
        )]
        .into(),
        time: Utc.with_ymd_and_hms(1970, 1, 1, 0, 0, 0).unwrap(),
        next_time: Utc.with_ymd_and_hms(1970, 1, 1, 0, 0, 0).unwrap(),
        market_time: MarketTime::Regular,

        price_histories: HashMap::new(),
        price_history_start: Utc.with_ymd_and_hms(1970, 1, 1, 0, 0, 0).unwrap(),
        price_history_interval: TimeDelta::minutes(1),

        cash: 0.0,
        holdings: HashMap::new(),
    };

    let _ = market.next_event_or_tick(TimeDelta::minutes(1)).await;

    assert_event(
        Event::PostMarketEnd,
        Utc.with_ymd_and_hms(1970, 1, 1, 0, 1, 0).unwrap(),
        market.next_event_or_tick(TimeDelta::minutes(1)).await,
    );
}

#[tokio::test]
async fn test_prices() {
    let mut market = TestMarket {
        events: VecDeque::new(),
        time: Utc.with_ymd_and_hms(1970, 1, 1, 0, 0, 0).unwrap(),
        next_time: Utc.with_ymd_and_hms(1970, 1, 1, 0, 0, 0).unwrap(),
        market_time: MarketTime::Regular,

        price_histories: [("STOCK".to_string(), vec![10.0..11.0, 12.0..13.0])].into(),
        price_history_start: Utc.with_ymd_and_hms(1970, 1, 1, 0, 0, 0).unwrap(),
        price_history_interval: TimeDelta::minutes(1),

        cash: 0.0,
        holdings: HashMap::new(),
    };

    let (mut time, _) = market
        .next_event_or_tick(TimeDelta::minutes(1))
        .await
        .unwrap()
        .unwrap();

    assert_in_range(10.0, 11.0, market.price_at("STOCK", time).unwrap());
    assert_in_range(10.0, 11.0, market.current_price("STOCK").unwrap());

    (time, _) = market
        .next_event_or_tick(TimeDelta::minutes(1))
        .await
        .unwrap()
        .unwrap();

    assert_in_range(12.0, 13.0, market.price_at("STOCK", time).unwrap());
    assert_in_range(12.0, 13.0, market.current_price("STOCK").unwrap());
}

#[tokio::test]
async fn test_consistant_prices() {
    let mut market = TestMarket {
        events: VecDeque::new(),
        time: Utc.with_ymd_and_hms(1970, 1, 1, 0, 0, 0).unwrap(),
        next_time: Utc.with_ymd_and_hms(1970, 1, 1, 0, 0, 0).unwrap(),
        market_time: MarketTime::Regular,

        price_histories: [("STOCK".to_string(), vec![10.0..10.0])].into(),
        price_history_start: Utc.with_ymd_and_hms(1970, 1, 1, 0, 0, 0).unwrap(),
        price_history_interval: TimeDelta::minutes(1),

        cash: 0.0,
        holdings: HashMap::new(),
    };

    let _ = market
        .next_event_or_tick(TimeDelta::minutes(1))
        .await
        .unwrap()
        .unwrap();

    assert_float_eq!(10.0, market.current_price("STOCK").unwrap(), ulps <= 5);
}

#[tokio::test]
#[should_panic]
async fn test_inverted_lows_and_highs() {
    let mut market = TestMarket {
        events: VecDeque::new(),
        time: Utc.with_ymd_and_hms(1970, 1, 1, 0, 0, 0).unwrap(),
        next_time: Utc.with_ymd_and_hms(1970, 1, 1, 0, 0, 0).unwrap(),
        market_time: MarketTime::Regular,

        price_histories: [("STOCK".to_string(), vec![11.0..10.0])].into(),
        price_history_start: Utc.with_ymd_and_hms(1970, 1, 1, 0, 0, 0).unwrap(),
        price_history_interval: TimeDelta::minutes(1),

        cash: 0.0,
        holdings: HashMap::new(),
    };

    let _ = market
        .next_event_or_tick(TimeDelta::minutes(1))
        .await
        .unwrap()
        .unwrap();

    let _ = market.current_price("STOCK").unwrap();
}

#[tokio::test]
#[should_panic]
async fn test_future_prices() {
    let mut market = TestMarket {
        events: VecDeque::new(),
        time: Utc.with_ymd_and_hms(1970, 1, 1, 0, 0, 0).unwrap(),
        next_time: Utc.with_ymd_and_hms(1970, 1, 1, 0, 0, 0).unwrap(),
        market_time: MarketTime::Regular,

        price_histories: [("STOCK".to_string(), vec![10.0..11.0, 12.0..13.0])].into(),
        price_history_start: Utc.with_ymd_and_hms(1970, 1, 1, 0, 0, 0).unwrap(),
        price_history_interval: TimeDelta::minutes(1),

        cash: 0.0,
        holdings: HashMap::new(),
    };

    let _ = market
        .next_event_or_tick(TimeDelta::minutes(1))
        .await
        .unwrap()
        .unwrap();

    let _ = market.price_at("STOCK", Utc.with_ymd_and_hms(1970, 1, 1, 0, 1, 0).unwrap());
}

#[tokio::test]
async fn test_buy_and_sell() {
    let mut market = TestMarket {
        events: VecDeque::new(),
        time: Utc.with_ymd_and_hms(1970, 1, 1, 0, 0, 0).unwrap(),
        next_time: Utc.with_ymd_and_hms(1970, 1, 1, 0, 0, 0).unwrap(),
        market_time: MarketTime::Regular,

        price_histories: [("STOCK".to_string(), vec![1.0..1.0, 2.0..2.0])].into(),
        price_history_start: Utc.with_ymd_and_hms(1970, 1, 1, 0, 0, 0).unwrap(),
        price_history_interval: TimeDelta::minutes(1),

        cash: 100.0,
        holdings: HashMap::new(),
    };

    let _ = market
        .next_event_or_tick(TimeDelta::minutes(1))
        .await
        .unwrap()
        .unwrap();

    market.buy_at_market("STOCK", 100);

    assert_float_eq!(0.0, market.cash, ulps <= 5);
    assert_eq!(100, *market.holdings.get("STOCK").unwrap());

    let _ = market
        .next_event_or_tick(TimeDelta::minutes(1))
        .await
        .unwrap()
        .unwrap();

    market.sell_at_market("STOCK", 100);

    assert_float_eq!(200.0, market.cash, ulps <= 5);
}

#[tokio::test]
#[should_panic]
async fn test_buy_more_than_cash() {
    let mut market = TestMarket {
        events: VecDeque::new(),
        time: Utc.with_ymd_and_hms(1970, 1, 1, 0, 0, 0).unwrap(),
        next_time: Utc.with_ymd_and_hms(1970, 1, 1, 0, 0, 0).unwrap(),
        market_time: MarketTime::Regular,

        price_histories: [("STOCK".to_string(), vec![1.0..1.0, 2.0..2.0])].into(),
        price_history_start: Utc.with_ymd_and_hms(1970, 1, 1, 0, 0, 0).unwrap(),
        price_history_interval: TimeDelta::minutes(1),

        cash: 100.0,
        holdings: HashMap::new(),
    };

    let _ = market
        .next_event_or_tick(TimeDelta::minutes(1))
        .await
        .unwrap()
        .unwrap();

    market.buy_at_market("STOCK", 101);
}

#[tokio::test]
#[should_panic]
async fn test_sell_more_than_holdings() {
    let mut market = TestMarket {
        events: VecDeque::new(),
        time: Utc.with_ymd_and_hms(1970, 1, 1, 0, 0, 0).unwrap(),
        next_time: Utc.with_ymd_and_hms(1970, 1, 1, 0, 0, 0).unwrap(),
        market_time: MarketTime::Regular,

        price_histories: [("STOCK".to_string(), vec![1.0..1.0, 2.0..2.0])].into(),
        price_history_start: Utc.with_ymd_and_hms(1970, 1, 1, 0, 0, 0).unwrap(),
        price_history_interval: TimeDelta::minutes(1),

        cash: 100.0,
        holdings: HashMap::new(),
    };

    let _ = market
        .next_event_or_tick(TimeDelta::minutes(1))
        .await
        .unwrap()
        .unwrap();

    market.buy_at_market("STOCK", 100);

    let _ = market
        .next_event_or_tick(TimeDelta::minutes(1))
        .await
        .unwrap()
        .unwrap();

    market.sell_at_market("STOCK", 101);
}
