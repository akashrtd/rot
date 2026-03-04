use futures::StreamExt;
use rot_provider::{
    new_openai_provider, Provider, ProviderContent, ProviderMessage, Request, StreamEvent,
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
        r#"data: {"id":"chatcmpl-123","object":"chat.completion.chunk","created":1694268190,"model":"gpt-4o","choices":[{"index":0,"delta":{"role":"assistant","content":""},"finish_reason":null}]}"#,
        "",
        r#"data: {"id":"chatcmpl-123","object":"chat.completion.chunk","created":1694268190,"model":"gpt-4o","choices":[{"index":0,"delta":{"content":"Hello"},"finish_reason":null}]}"#,
        "",
        r#"data: {"id":"chatcmpl-123","object":"chat.completion.chunk","created":1694268190,"model":"gpt-4o","choices":[{"index":0,"delta":{"content":" world"},"finish_reason":null}]}"#,
        "",
        r#"data: {"id":"chatcmpl-123","object":"chat.completion.chunk","created":1694268190,"model":"gpt-4o","choices":[{"index":0,"delta":{},"finish_reason":"stop"}],"usage":{"prompt_tokens":11,"completion_tokens":2,"total_tokens":13}}"#,
        "",
        r#"data: [DONE]"#,
        "",
    ]
    .join("\n")
}

async fn mount_openai_stream_mock(server: &MockServer) {
    let body = sample_sse_response();

    Mock::given(method("POST"))
        .and(path("/v1/chat/completions"))
        .and(header("Authorization", "Bearer test-key"))
        .and(body_partial_json(json!({
            "model": "gpt-4o",
            "stream": true,
        })))
        .respond_with(ResponseTemplate::new(200).set_body_raw(body, "text/event-stream"))
        .mount(server)
        .await;

    let complete_body = json!({
        "id": "chatcmpl-123",
        "object": "chat.completion",
        "created": 1694268190,
        "model": "gpt-4o",
        "choices": [{
            "index": 0,
            "message": {
                "role": "assistant",
                "content": "Hello world"
            },
            "finish_reason": "stop"
        }],
        "usage": {
            "prompt_tokens": 11,
            "completion_tokens": 2,
            "total_tokens": 13
        }
    });

    Mock::given(method("POST"))
        .and(path("/v1/chat/completions"))
        .and(header("Authorization", "Bearer test-key"))
        .and(body_partial_json(json!({
            "model": "gpt-4o",
            "stream": false,
        })))
        .respond_with(ResponseTemplate::new(200).set_body_json(complete_body))
        .mount(server)
        .await;
}

#[tokio::test]
async fn test_stream_with_wiremock_sse() {
    let server = MockServer::start().await;
    mount_openai_stream_mock(&server).await;

    // We instantiate generic openai_compat but with openai's default
    let provider = new_openai_provider("test-key".to_string()).with_base_url(&format!("{}/v1", server.uri()));
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
    mount_openai_stream_mock(&server).await;

    let provider = new_openai_provider("test-key".to_string()).with_base_url(&format!("{}/v1", server.uri()));
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
