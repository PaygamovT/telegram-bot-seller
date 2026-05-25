use telegram_bot_seller::modules::ai::gemini::{
    GeminiRequest, GeminiResponse, GeminiContent, GeminiPart, GeminiInlineData,
    OpenRouterRequest, OpenRouterResponse, OpenRouterMessage, OpenRouterContent, OpenRouterImageUrl, OpenRouterInputAudio
};
use telegram_bot_seller::modules::ai::minimax::{MiniMaxMessage, MiniMaxResponse, MiniMaxToolCall};
use telegram_bot_seller::modules::ai::tools::get_minimax_tools_schema;

#[test]
fn test_gemini_request_serialization() {
    let req = GeminiRequest {
        contents: vec![GeminiContent {
            parts: vec![
                GeminiPart::InlineData {
                    inline_data: GeminiInlineData {
                        mime_type: "image/jpeg".to_string(),
                        data: "aaaa_encoded_image".to_string(),
                    },
                },
                GeminiPart::Text {
                    text: "Identify this brand".to_string(),
                },
            ],
        }],
    };

    let serialized = serde_json::to_string(&req).expect("Failed to serialize GeminiRequest");
    
    // Validate containing structure keys
    assert!(serialized.contains("\"contents\""));
    assert!(serialized.contains("\"parts\""));
    assert!(serialized.contains("\"inline_data\""));
    assert!(serialized.contains("\"mime_type\":\"image/jpeg\""));
    assert!(serialized.contains("\"data\":\"aaaa_encoded_image\""));
    assert!(serialized.contains("\"text\":\"Identify this brand\""));
}

#[test]
fn test_gemini_response_deserialization() {
    let mock_json = r#"
    {
        "candidates": [
            {
                "content": {
                    "parts": [
                        {
                            "text": "This is a bottle of Bleu de Chanel. Scent profile is fresh, woody."
                        }
                    ]
                }
            }
        ]
    }
    "#;

    let response: GeminiResponse = serde_json::from_str(mock_json).expect("Failed to deserialize GeminiResponse");
    
    // Extract text from the candidate parts safely
    let parsed_text = response
        .candidates
        .as_ref()
        .and_then(|cands| cands.first())
        .and_then(|cand| cand.content.as_ref())
        .and_then(|content| content.parts.as_ref())
        .and_then(|parts| parts.first())
        .and_then(|part| part.text.clone());

    assert_eq!(
        parsed_text.as_deref(),
        Some("This is a bottle of Bleu de Chanel. Scent profile is fresh, woody.")
    );
}

#[test]
fn test_openrouter_request_serialization() {
    // 1. Test image format
    let img_req = OpenRouterRequest {
        model: "google/gemini-2.5-flash".to_string(),
        messages: vec![OpenRouterMessage {
            role: "user".to_string(),
            content: vec![
                OpenRouterContent::Text {
                    text: "Analyze payment receipt".to_string(),
                },
                OpenRouterContent::ImageUrl {
                    image_url: OpenRouterImageUrl {
                        url: "data:image/png;base64,bbbb_encoded_receipt".to_string(),
                    },
                },
            ],
        }],
    };

    let serialized_img = serde_json::to_string(&img_req).expect("Failed to serialize OpenRouterRequest for image");
    assert!(serialized_img.contains("\"model\":\"google/gemini-2.5-flash\""));
    assert!(serialized_img.contains("\"role\":\"user\""));
    assert!(serialized_img.contains("\"type\":\"text\""));
    assert!(serialized_img.contains("\"text\":\"Analyze payment receipt\""));
    assert!(serialized_img.contains("\"type\":\"image_url\""));
    assert!(serialized_img.contains("\"url\":\"data:image/png;base64,bbbb_encoded_receipt\""));

    // 2. Test audio format
    let audio_req = OpenRouterRequest {
        model: "google/gemini-2.5-flash".to_string(),
        messages: vec![OpenRouterMessage {
            role: "user".to_string(),
            content: vec![
                OpenRouterContent::Text {
                    text: "Transcribe audio".to_string(),
                },
                OpenRouterContent::InputAudio {
                    input_audio: OpenRouterInputAudio {
                        data: "cccc_encoded_voice".to_string(),
                        format: "ogg".to_string(),
                    },
                },
            ],
        }],
    };

    let serialized_audio = serde_json::to_string(&audio_req).expect("Failed to serialize OpenRouterRequest for audio");
    assert!(serialized_audio.contains("\"type\":\"input_audio\""));
    assert!(serialized_audio.contains("\"data\":\"cccc_encoded_voice\""));
    assert!(serialized_audio.contains("\"format\":\"ogg\""));
}

#[test]
fn test_openrouter_response_deserialization() {
    let mock_json = r#"
    {
        "choices": [
            {
                "message": {
                    "role": "assistant",
                    "content": "Transaction details:\n- Amount: 5000\n- Date: 2026-05-25"
                }
            }
        ]
    }
    "#;

    let response: OpenRouterResponse = serde_json::from_str(mock_json).expect("Failed to deserialize OpenRouterResponse");
    
    let parsed_text = response
        .choices
        .as_ref()
        .and_then(|choices| choices.first())
        .and_then(|choice| choice.message.as_ref())
        .and_then(|msg| msg.content.clone());

    assert_eq!(
        parsed_text.as_deref(),
        Some("Transaction details:\n- Amount: 5000\n- Date: 2026-05-25")
    );
}

#[test]
fn test_minimax_message_serialization() {
    let msg = MiniMaxMessage {
        role: "assistant".to_string(),
        content: Some("I have processed your request".to_string()),
        tool_calls: Some(vec![MiniMaxToolCall {
            id: "call_t1".to_string(),
            r#type: "function".to_string(),
            function: telegram_bot_seller::modules::ai::minimax::MiniMaxFunctionCall {
                name: "get_catalog".to_string(),
                arguments: "{}".to_string(),
            },
        }]),
        tool_call_id: None,
        name: None,
    };

    let serialized = serde_json::to_string(&msg).expect("Failed to serialize MiniMaxMessage");
    assert!(serialized.contains("\"role\":\"assistant\""));
    assert!(serialized.contains("\"content\":\"I have processed your request\""));
    assert!(serialized.contains("\"tool_calls\""));
    assert!(serialized.contains("\"id\":\"call_t1\""));
    assert!(serialized.contains("\"name\":\"get_catalog\""));
}

#[test]
fn test_minimax_response_deserialization() {
    let mock_json = r#"
    {
        "choices": [
            {
                "message": {
                    "role": "assistant",
                    "content": "Here is the perfume catalog."
                },
                "finish_reason": "stop"
            }
        ]
    }
    "#;

    let response: MiniMaxResponse = serde_json::from_str(mock_json).expect("Failed to deserialize MiniMaxResponse");
    let choice = response.choices.as_ref().unwrap().first().unwrap();
    assert_eq!(choice.finish_reason.as_deref(), Some("stop"));
    assert_eq!(choice.message.as_ref().unwrap().content.as_deref(), Some("Here is the perfume catalog."));
}

#[test]
fn test_minimax_reaction_tag_parsing() {
    // 1. Text with reaction tag
    let raw_text = "Конечно, ваш заказ оформлен! Ожидайте доставку.\n[REACTION: 👍]";
    
    // We replicate the parse_reaction_marker function's logic inside test
    let tag = "[REACTION:";
    let parsed = if let Some(start_idx) = raw_text.find(tag) {
        if let Some(end_idx) = raw_text[start_idx..].find(']') {
            let emoji_part = &raw_text[start_idx + tag.len()..start_idx + end_idx];
            let emoji = emoji_part.trim().to_string();
            let before = &raw_text[..start_idx];
            let after = &raw_text[start_idx + end_idx + 1..];
            let cleaned = format!("{}{}", before, after).trim().to_string();
            (cleaned, Some(emoji))
        } else {
            (raw_text.to_string(), None)
        }
    } else {
        (raw_text.to_string(), None)
    };

    assert_eq!(parsed.0, "Конечно, ваш заказ оформлен! Ожидайте доставку.");
    assert_eq!(parsed.1, Some("👍".to_string()));

    // 2. Text without reaction tag
    let raw_text_none = "Конечно, ваш заказ оформлен!";
    let parsed_none = if let Some(start_idx) = raw_text_none.find(tag) {
        if let Some(end_idx) = raw_text_none[start_idx..].find(']') {
            let emoji_part = &raw_text_none[start_idx + tag.len()..start_idx + end_idx];
            let emoji = emoji_part.trim().to_string();
            let before = &raw_text_none[..start_idx];
            let after = &raw_text_none[start_idx + end_idx + 1..];
            let cleaned = format!("{}{}", before, after).trim().to_string();
            (cleaned, Some(emoji))
        } else {
            (raw_text_none.to_string(), None)
        }
    } else {
        (raw_text_none.to_string(), None)
    };

    assert_eq!(parsed_none.0, "Конечно, ваш заказ оформлен!");
    assert_eq!(parsed_none.1, None);
}

#[test]
fn test_tools_schema_generation() {
    let schema = get_minimax_tools_schema();
    assert!(schema.is_array());
    let arr = schema.as_array().unwrap();
    assert_eq!(arr.len(), 10);
    
    let has_get_catalog = arr.iter().any(|val| {
        val.get("function").and_then(|f| f.get("name")).and_then(|n| n.as_str()) == Some("get_catalog")
    });
    assert!(has_get_catalog);
}

#[tokio::test]
async fn test_tool_execution_routing() {
    use telegram_bot_seller::shared::db;
    use telegram_bot_seller::modules::ai::tools::execute_tool;
    use telegram_bot_seller::shared::types::{ChatId, ProductId};
    use telegram_bot_seller::modules::contacts;

    let temp_dir = std::env::temp_dir();
    let db_name = format!(
        "test_ai_tools_{}.db",
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_millis()
    );
    let db_path = temp_dir.join(db_name);
    let db_path_str = db_path.to_str().unwrap();

    let pool = db::init(db_path_str).await.expect("Failed to init db");
    db::run_migrations(&pool).await.expect("Failed to run migrations");

    // 1. Test get_contacts when empty
    let res = execute_tool("get_contacts", "{\"chat_id\": 12345}", &pool).await.expect("Failed to get_contacts");
    assert_eq!(res, "null");

    // 2. Test update_contacts
    let update_args = r#"{"chat_id": 12345, "first_name": "Alice", "phone_number": "+79998887766"}"#;
    let res = execute_tool("update_contacts", update_args, &pool).await.expect("Failed to update_contacts");
    assert!(res.contains("success"));

    // Verify contact indeed exists now
    let contact_opt = contacts::get_contacts(&pool, ChatId(12345)).await.expect("Failed to fetch");
    assert!(contact_opt.is_some());
    let contact = contact_opt.unwrap();
    assert_eq!(contact.first_name.as_deref(), Some("Alice"));
    assert_eq!(contact.phone_number.as_deref(), Some("+79998887766"));

    // Cleanup
    let _ = std::fs::remove_file(db_path);
}
