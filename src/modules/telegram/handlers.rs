use crate::shared::db::DbPool;
use crate::shared::config::AppConfig;
use crate::shared::error::AppResult;
use crate::shared::types::ChatId;
use crate::modules::contacts;
use crate::modules::telegram::business::{Message, send_message, send_reaction};
use crate::modules::telegram::media::download_telegram_file;
use reqwest::Client;
use tracing::{debug, error, info, warn};

pub static RATE_LIMITER: std::sync::OnceLock<crate::shared::rate_limiter::RateLimiter> = std::sync::OnceLock::new();

pub async fn handle_message_update(
    client: &Client,
    pool: &DbPool,
    config: &AppConfig,
    message: &Message,
) -> AppResult<()> {
    let active_config = config.load_dynamic(pool).await;
    let chat_id = ChatId(message.chat.id);
    
    // Sliding-Window Rate Limiting (default: 5 requests per 30 seconds per user)
    let limiter = RATE_LIMITER.get_or_init(|| {
        crate::shared::rate_limiter::RateLimiter::new(5, std::time::Duration::from_secs(30))
    });
    if !limiter.check(message.chat.id) {
        warn!("[Telegram.handler] Rate limit exceeded for user {}", message.chat.id);
        let limit_reply = "⚠️ *Внимание*!\nВы отправляете сообщения слишком часто. Пожалуйста, подождите несколько секунд перед следующим запросом.";
        send_message(client, &active_config.telegram_token, message.chat.id, limit_reply, message.business_connection_id.as_deref()).await?;
        return Ok(());
    }

    info!(
        "[Telegram.handler] Processing message from user {} (chat_id: {})",
        message.chat.first_name.as_deref().unwrap_or("Unknown"),
        chat_id
    );

    // 1. Upsert contact information to catalog customer record
    let new_contact = contacts::Contact {
        chat_id,
        first_name: message.chat.first_name.clone(),
        address: None,
        phone_number: None,
        username: message.chat.username.clone(),
        nickname: None,
    };
    if let Err(err) = contacts::update_contacts(pool, &new_contact).await {
        warn!("[Telegram.handler] Failed to save contact {chat_id} in database: {err}");
    } else {
        debug!("[Telegram.handler] Successfully saved contact {chat_id} in database");
    }

    // 2. Put reaction (like/thumbs up) to incoming message to demonstrate Business API
    if let Err(err) = send_reaction(client, &active_config.telegram_token, message.chat.id, message.message_id, "👍", message.business_connection_id.as_deref()).await {
        warn!("[Telegram.handler] Failed to send reaction to message {}: {err}", message.message_id);
    }

    // 3. Route updates based on content types
    if let Some(voice) = &message.voice {
        let file_id = &voice.file_id;
        let dest = format!("./data/downloads/voice/{file_id}.ogg");
        
        info!("[Telegram.handler] Received voice message. Initiating download to: {dest}");
        if let Err(err) = download_telegram_file(client, &active_config.telegram_token, file_id, &dest).await {
            warn!("[Telegram.handler] Failed to download voice file: {err}");
        }

        // 1. Transcribe voice note
        let transcription_res = crate::modules::ai::transcribe_voice(client, &active_config, &dest).await;
        
        let reply = match transcription_res {
            Ok(transcription) => {
                debug!("[Telegram.handler] Voice note transcribed: \"{transcription}\"");

                // 2. Feed text into conversational dialogue engine
                match crate::modules::ai::run_dialog(client, &active_config, pool, chat_id, &transcription).await {
                    Ok((dialog_reply, reaction_opt)) => {
                        let final_reply = format!(
                            "🎤 *Транскрипция*: \"{}\"\n\n🤖 *Ответ продавца*:\n{}",
                            transcription, dialog_reply
                        );
                        send_message(client, &active_config.telegram_token, message.chat.id, &final_reply, message.business_connection_id.as_deref()).await?;
                        reaction_opt
                    }
                    Err(err) => {
                        let err_msg = format!("MiniMax dialog failed during voice processing: {err}");
                        error!("[Telegram.handler] {err_msg}");
                        let _ = crate::shared::alerting::send_alert(&err_msg).await;
                        
                        let backup_reply = "🤖 *Ответ продавца*:\nИзвините, мой модуль консультаций временно перегружен. Я уже уведомил владельца. Пожалуйста, попробуйте написать чуть позже или свяжитесь с нами напрямую!";
                        send_message(client, &active_config.telegram_token, message.chat.id, backup_reply, message.business_connection_id.as_deref()).await?;
                        None
                    }
                }
            }
            Err(err) => {
                let err_msg = format!("Gemini voice note transcription failed: {err}");
                warn!("[Telegram.handler] {err_msg}");
                let _ = crate::shared::alerting::send_alert(&err_msg).await;
                
                let backup_reply = "🎤 *Голосовое сообщение получено*!\n\n🤖 *Ответ продавца*:\nИзвините, сейчас мои системы распознавания голоса временно недоступны. Пожалуйста, напишите ваш запрос текстом, и я с радостью отвечу вам!";
                send_message(client, &active_config.telegram_token, message.chat.id, backup_reply, message.business_connection_id.as_deref()).await?;
                None
            }
        };

        if let Some(emoji) = reply {
            let _ = send_reaction(client, &active_config.telegram_token, message.chat.id, message.message_id, &emoji, message.business_connection_id.as_deref()).await;
        }
    } else if let Some(photos) = &message.photo {
        if let Some(largest_photo) = photos.iter().max_by_key(|p| p.file_size.unwrap_or(0)) {
            let file_id = &largest_photo.file_id;
            let dest = format!("./data/downloads/photos/{file_id}.jpg");

            info!("[Telegram.handler] Received photo. Initiating download to: {dest}");
            if let Err(err) = download_telegram_file(client, &active_config.telegram_token, file_id, &dest).await {
                warn!("[Telegram.handler] Failed to download photo file: {err}");
            }

            // 1. Describe product / OCR receipt
            let prompt = "Analyze this image. If it is a payment receipt, bank transfer screenshot, or payment proof, perform OCR and extract: Sender name, Transaction Amount, Date/Time, Status, and Transaction ID. If it is a photo of a perfume, fragrance bottle, or product packaging, describe the product, brand name, bottle style, and any scent characteristics or visible text. Keep the response clean and well-structured.";
            let analysis_res = crate::modules::ai::analyze_image(client, &active_config, &dest, prompt).await;

            let reply = match analysis_res {
                Ok(analysis) => {
                    debug!("[Telegram.handler] Photo analysis/OCR result: \"{analysis}\"");

                    // 2. Feed visual/OCR content to conversational dialogue engine
                    match crate::modules::ai::run_dialog(client, &active_config, pool, chat_id, &analysis).await {
                        Ok((dialog_reply, reaction_opt)) => {
                            let final_reply = format!(
                                "📸 *Анализ изображения*:\n{}\n\n🤖 *Ответ продавца*:\n{}",
                                analysis, dialog_reply
                            );
                            send_message(client, &active_config.telegram_token, message.chat.id, &final_reply, message.business_connection_id.as_deref()).await?;
                            reaction_opt
                        }
                        Err(err) => {
                            let err_msg = format!("MiniMax dialog failed during photo processing: {err}");
                            error!("[Telegram.handler] {err_msg}");
                            let _ = crate::shared::alerting::send_alert(&err_msg).await;
                            
                            let backup_reply = "🤖 *Ответ продавца*:\nИзвините, мой модуль консультаций временно перегружен. Я уже уведомил владельца. Пожалуйста, попробуйте написать чуть позже или свяжитесь с нами напрямую!";
                            send_message(client, &active_config.telegram_token, message.chat.id, backup_reply, message.business_connection_id.as_deref()).await?;
                            None
                        }
                    }
                }
                Err(err) => {
                    let err_msg = format!("Gemini visual photo analysis failed: {err}");
                    warn!("[Telegram.handler] {err_msg}");
                    let _ = crate::shared::alerting::send_alert(&err_msg).await;
                    
                    let backup_reply = "📸 *Изображение получено*!\n\n🤖 *Ответ продавца*:\nИзвините, сейчас мои системы визуального анализа временно недоступны. Пожалуйста, опишите текстом, что изображено на фото, или продублируйте информацию, и я помогу вам!";
                    send_message(client, &active_config.telegram_token, message.chat.id, backup_reply, message.business_connection_id.as_deref()).await?;
                    None
                }
            };

            if let Some(emoji) = reply {
                let _ = send_reaction(client, &active_config.telegram_token, message.chat.id, message.message_id, &emoji, message.business_connection_id.as_deref()).await;
            }
        }
    } else if let Some(text) = &message.text {
        info!("[Telegram.handler] Received text message: \"{text}\"");
        
        let (reply, reaction_opt) = match crate::modules::ai::run_dialog(client, &active_config, pool, ChatId(message.chat.id), text).await {
            Ok(res) => res,
            Err(err) => {
                let err_msg = format!("MiniMax dialog failed during text processing: {err}");
                error!("[Telegram.handler] {err_msg}");
                let _ = crate::shared::alerting::send_alert(&err_msg).await;
                
                (
                    "🤖 *Ответ продавца*:\nИзвините, мой модуль консультаций временно перегружен. Я уже уведомил владельца. Пожалуйста, попробуйте написать чуть позже или свяжитесь с нами напрямую!".to_string(),
                    None
                )
            }
        };

        send_message(client, &active_config.telegram_token, message.chat.id, &reply, message.business_connection_id.as_deref()).await?;
        
        if let Some(emoji) = reaction_opt {
            debug!("[Telegram.handler] Automated reaction decided by AI: {emoji}");
            if let Err(err) = send_reaction(client, &active_config.telegram_token, message.chat.id, message.message_id, &emoji, message.business_connection_id.as_deref()).await {
                warn!("[Telegram.handler] Failed to send reaction to message {}: {err}", message.message_id);
            }
        }
    } else {
        info!("[Telegram.handler] Received unsupported attachment type");
        let reply = "Unsupported attachment type.\n\nOnly text, photo, and voice formats are processed by the sales pipeline.";
        send_message(client, &active_config.telegram_token, message.chat.id, reply, message.business_connection_id.as_deref()).await?;
    }

    Ok(())
}
