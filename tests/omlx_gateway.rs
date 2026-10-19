//! Socket-free serialization compatibility; dispatcher coverage lives in src/cli/gateway_requests.rs.
use serde_json::json;

#[test]
fn native_reasoning_history_and_xhigh_round_trip() {
    use mojentic::llm::gateway::ReasoningEffort;
    use mojentic::llm::models::LlmMessage;

    let message: LlmMessage = serde_json::from_value(json!({
        "role": "assistant", "content": "Visible answer",
        "reasoning_content": "Reasoning sentinel"
    }))
    .unwrap();
    let serialized = serde_json::to_value(message).unwrap();
    assert_eq!(serialized["content"], "Visible answer");
    assert_eq!(serialized["reasoning_content"], "Reasoning sentinel");
    assert!(serialized.get("thinking").is_none());
    assert_eq!(
        serde_json::from_value::<ReasoningEffort>(json!("xhigh")).unwrap(),
        ReasoningEffort::XHigh
    );
}
