// use mmatamm_interface::influxdb_market::InfluxDbMarket;

use std::error::Error;

use chrono::{DateTime, TimeDelta, Utc};
use mmatamm_interface::{market::Market, questdb_market::QuestDbMarket};
use tokio_postgres::NoTls;

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    flexi_logger::init();

    // Connect to the database
    let (client, connection) = tokio_postgres::connect(
        "user=admin password=quest host=localhost port=8812 dbname=qdb",
        NoTls,
    )
    .await?;

    // The connection object performs the actual communication with the database,
    // so spawn it off to run on its own.
    tokio::spawn(async move {
        if let Err(e) = connection.await {
            eprintln!("connection error: {}", e);
        }
    });

    // Now we can execute a simple statement that just returns its parameter.
    // let rows = client
    //     .query(
    //         "SELECT * FROM ticks WHERE timestamp >= '2024-06-30' AND symbol = $1::TEXT;",
    //         &[&"QQQM"],
    //     )
    //     .await?;

    // // And then check that we got back the same string we sent over.
    // let value: SystemTime = rows[1].get(5);
    // let cooler: DateTime<Utc> = value.into();
    // println!("{}", cooler);
    // assert_eq!(value, "hello world");

    let mut market = QuestDbMarket::new(
        &client,
        "2024-06-25T12:12:12Z".parse::<DateTime<Utc>>()?,
        10_000.0,
    )
    .await?;

    for _ in 0..40 {
        println!(
            "{:?} | {:?}",
            market.next_event_or_tick(TimeDelta::hours(1)).await?,
            market.market_time()
        );

        let price = market.current_price("QQQM").await?;
        // let price = market
        //     .price_at("PLTR", market.time() + TimeDelta::hours(1))
        //     .await?;
        println!("{:?}", price);
    }

    Ok(())

    // println!("Connected!");

    // let market = InfluxDbMarket::new(client);
    // market.cool().await.unwrap();
}
