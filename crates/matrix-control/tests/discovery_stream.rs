use matrix_control::discovery::{discovery, discovery_for_each};
use serde_json::json;

#[test]
fn streamed_discovery_matches_legacy_batch_and_stops_on_error() {
    let catalog = json!([{"id":"test", "name":"Test", "parameters":[
        {"id":"speed", "label":"Speed", "kind":"number", "min":0,"max":100,"step":1},
        {"id":"enabled", "kind":"boolean"},
        {"id":"palette", "kind":"select", "options":["A","B"]}
    ]}]);
    let expected = discovery("stream_test", &catalog);
    let mut received = Vec::new();
    discovery_for_each("stream_test", &catalog, |record| {
        received.push(record);
        Ok::<(), &str>(())
    })
    .unwrap();
    assert_eq!(expected, received);
    let mut calls = 0;
    let result = discovery_for_each("stream_test", &catalog, |_| {
        calls += 1;
        if calls == 3 {
            Err("stop")
        } else {
            Ok(())
        }
    });
    assert_eq!(result, Err("stop"));
    assert_eq!(calls, 3);
}
