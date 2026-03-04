use futures::StreamExt;
use rot_provider::{
    AnthropicProvider, Provider, ProviderContent, ProviderMessage, Request, StreamEvent,
};
use serde_json::json;
use wiremock::matchers::{body_partial_json, header, method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

fn sample_request() -> Request {
    Request {
        messages: vec![ProviderMessage {
            role: "user".to_string(),
            content: vec![ProviderContent::Text {
                text: "Say hello".to_string(),
            }],
        }],
        tools: vec![],
        system: Some("You are helpful".to_string()),
        max_tokens: Some(64),
        thinking: None,
    }
}

fn sample_sse_response() -> String {
    [
        r#"data: {"type":"message_start","message":{"usage":{"input_tokens":11,"output_tokens":0}}}"#,
        "",
        r#"data: {"type":"content_block_start","index":0,"content_block":{"type":"text","text":""}}"#,
        "",
        r#"data: {"type":"content_block_delta","index":0,"delta":{"type":"text_delta","text":"Hello"}}"#,
        "",
        r#"data: {"type":"content_block_delta","index":0,"delta":{"type":"text_delta","text":" world"}}"#,
        "",
        r#"data: {"type":"message_delta","delta":{"stop_reason":"end_turn"},"usage":{"input_tokens":11,"output_tokens":2}}"#,
        "",
        r#"data: {"type":"message_stop"}"#,
        "",
    ]
    .join("\n")
}

async fn mount_anthropic_stream_mock(server: &MockServer) {
    let body = sample_sse_response();

    Mock::given(method("POST"))
        .and(path("/v1/messages"))
        .and(header("x-api-key", "test-key"))
        .and(header("anthropic-version", "2023-06-01"))
        .and(body_partial_json(json!({
            "model": "claude-sonnet-4-20250514",
            "stream": true,
        })))
        .respond_with(ResponseTemplate::new(200).set_body_raw(body, "text/event-stream"))
        .expect(1)
        .mount(server)
        .await;
}

#[tokio::test]
async fn test_stream_with_wiremock_sse() {
    let server = MockServer::start().await;
    mount_anthropic_stream_mock(&server).await;

    let provider = AnthropicProvider::new("test-key").with_base_url(server.uri());
    let mut stream = provider
        .stream(sample_request())
        .await
        .expect("stream should start");

    let mut text = String::new();
    let mut saw_usage = false;
    let mut saw_done = false;

    while let Some(event) = stream.next().await {
        match event.expect("stream event should parse") {
            StreamEvent::TextDelta { delta } => text.push_str(&delta),
            StreamEvent::Usage { input, output } => {
                if input == 11 && output == 2 {
                    saw_usage = true;
                }
            }
            StreamEvent::Done { .. } => {
                saw_done = true;
                break;
            }
            _ => {}
        }
    }

    assert_eq!(text, "Hello world");
    assert!(saw_usage, "expected final usage update from SSE stream");
    assert!(saw_done, "expected done event from SSE stream");
}

#[tokio::test]
async fn test_complete_with_wiremock_sse() {
    let server = MockServer::start().await;
    mount_anthropic_stream_mock(&server).await;

    let provider = AnthropicProvider::new("test-key").with_base_url(server.uri());
    let response = provider
        .complete(sample_request())
        .await
        .expect("complete should succeed");

    let text = response
        .content
        .iter()
        .find_map(|block| match block {
            ProviderContent::Text { text } => Some(text.as_str()),
            _ => None,
        })
        .unwrap_or_default();

    assert_eq!(text, "Hello world");
    assert_eq!(response.usage.input_tokens, 11);
    assert_eq!(response.usage.output_tokens, 2);
}
