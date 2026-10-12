//! End-to-end test of the subscriber against a local ZeroMQ PUB socket standing in for the relay:
//! zlib frames -> decode -> dedupe -> database, `/test` and garbage frames, and the watchdog
//! reconnect after a silent period.

use nexus_eddn::{EddnConfig, EddnSubscriber};
use nexus_galaxy::{CommodityQuery, GalaxyDb};
use serde_json::{Value, json};
use std::io::Write;
use std::path::PathBuf;
use std::time::{Duration, Instant};
use zeromq::{Endpoint, PubSocket, Socket, SocketSend, ZmqMessage};

fn zlib(v: &Value) -> Vec<u8> {
    let mut e = flate2::write::ZlibEncoder::new(Vec::new(), flate2::Compression::default());
    e.write_all(v.to_string().as_bytes()).expect("zlib");
    e.finish().expect("zlib")
}

fn commodity(market_id: u64, station: &str, price: i64) -> Value {
    json!({
        "$schemaRef": "https://eddn.edcd.io/schemas/commodity/3",
        "header": {"uploaderID": "abc", "softwareName": "test", "softwareVersion": "1"},
        "message": {"systemName": "Sol", "stationName": station, "marketId": market_id,
                    "timestamp": nexus_journal_like_now(),
                    "commodities": [{"name": "gold", "meanPrice": 1, "buyPrice": price, "stock": 10, "stockBracket": 1,
                                     "sellPrice": 1, "demand": 0, "demandBracket": ""}]}
    })
}

/// ISO-8601 "now" without pulling in a date crate.
fn nexus_journal_like_now() -> String {
    let secs = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    let days = (secs / 86_400) as i64;
    let rem = secs % 86_400;
    // civil from days (Howard Hinnant)
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = if m <= 2 { y + 1 } else { y };
    format!(
        "{y:04}-{m:02}-{d:02}T{:02}:{:02}:{:02}Z",
        rem / 3600,
        (rem / 60) % 60,
        rem % 60
    )
}

fn wait_for(timeout: Duration, mut f: impl FnMut() -> bool) -> bool {
    let end = Instant::now() + timeout;
    while Instant::now() < end {
        if f() {
            return true;
        }
        std::thread::sleep(Duration::from_millis(50));
    }
    f()
}

#[test]
fn subscriber_ingests_from_a_local_relay_and_reconnects() {
    let rt = tokio::runtime::Builder::new_multi_thread()
        .worker_threads(2)
        .enable_all()
        .build()
        .expect("rt");
    let dir = PathBuf::from(env!("CARGO_TARGET_TMPDIR"))
        .join(format!("eddn-relay-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    let db = GalaxyDb::open(dir.join("galaxy.db")).expect("db");

    let (mut publisher, port) = rt.block_on(async {
        let mut p = PubSocket::new();
        let ep = p.bind("tcp://127.0.0.1:0").await.expect("bind");
        let port = match ep {
            Endpoint::Tcp(_, port) => port,
            other => panic!("unexpected endpoint {other:?}"),
        };
        (p, port)
    });
    let cfg = EddnConfig {
        endpoint: format!("tcp://127.0.0.1:{port}"),
        watchdog: Duration::from_secs(2),
        connect_timeout: Duration::from_secs(2),
        backoff_min: Duration::from_millis(100),
        ..EddnConfig::default()
    };
    let handle = EddnSubscriber::spawn_with(rt.handle(), db.clone(), cfg);
    let stats = handle.stats().clone();

    // A PUB socket drops messages until the subscription has arrived: repeat until one gets through.
    let first = zlib(&commodity(128016384, "Daedalus", 9000));
    let got = wait_for(Duration::from_secs(10), || {
        rt.block_on(publisher.send(ZmqMessage::from(first.clone())))
            .expect("send");
        stats.total_messages() > 0
    });
    assert!(got, "nothing received from the local relay");
    assert!(stats.is_connected());

    // Repeats of the same message are duplicates; /test and garbage frames are dropped.
    let mut test = commodity(1, "Test Station", 1);
    test["$schemaRef"] = json!("https://eddn.edcd.io/schemas/commodity/3/test");
    rt.block_on(async {
        publisher
            .send(ZmqMessage::from(first.clone()))
            .await
            .expect("send");
        publisher
            .send(ZmqMessage::from(zlib(&test)))
            .await
            .expect("send");
        publisher
            .send(ZmqMessage::from(b"garbage".to_vec()))
            .await
            .expect("send");
        publisher
            .send(ZmqMessage::from(zlib(&commodity(
                3700000000, "Second", 8000,
            ))))
            .await
            .expect("send");
    });
    assert!(wait_for(Duration::from_secs(5), || stats.total_messages()
        >= 2
        && stats.snapshot().decode_errors >= 1));
    let snap = stats.snapshot();
    assert_eq!(snap.total_messages, 2, "{snap:?}");
    assert!(snap.duplicates >= 1);
    assert_eq!(snap.test_dropped, 1);
    assert_eq!(snap.per_schema, vec![("commodity/3".to_string(), 2)]);
    assert!(
        snap.recent_headlines
            .iter()
            .all(|h| !h.text.contains("abc"))
    );

    db.flush().expect("flush");
    assert!(wait_for(Duration::from_secs(5), || {
        db.flush().is_ok() && db.station_market(3700000000).ok().flatten().is_some()
    }));
    let hits = db
        .search_commodity(&CommodityQuery {
            commodity: "gold".into(),
            ..Default::default()
        })
        .expect("q");
    assert_eq!(hits.len(), 2);
    assert_eq!(hits[0].price, 8000);

    // Silence for longer than the watchdog: the subscriber drops the socket and reconnects.
    assert!(
        wait_for(Duration::from_secs(10), || stats.snapshot().connects >= 2
            && stats.is_connected()),
        "{:?}",
        stats.snapshot()
    );
    // ...and receives again afterwards.
    let third = zlib(&commodity(3700000007, "Third", 7000));
    assert!(wait_for(Duration::from_secs(10), || {
        rt.block_on(publisher.send(ZmqMessage::from(third.clone())))
            .expect("send");
        stats.total_messages() >= 3
    }));

    drop(handle);
    assert!(wait_for(Duration::from_secs(5), || !stats.is_connected()));
    drop(publisher);
    drop(db);
    drop(rt);
    let _ = std::fs::remove_dir_all(&dir);
}
