//! Библиотека бизнес-логики для работы с LLM.
//! 
//! Этот модуль содержит переносимую логику, которая может быть использована:
//! - В CLI приложении (через main.rs)
//! - В WASM для веба
//! - В мобильном приложении как статическая библиотека
//! - В бэкенд-сервисе

use anyhow::{Context, Result};
use async_openai::{
    config::OpenAIConfig,
    types::{
        ChatCompletionRequestMessage,
        ChatCompletionRequestSystemMessageArgs,
        ChatCompletionRequestUserMessageArgs,
        ChatCompletionResponseStream,
        CreateChatCompletionRequest,
        CreateChatCompletionRequestArgs,
        ResponseFormat,
        Stop,
    },
    Client,
};

pub use async_openai::types::{
    ChatCompletionRequestAssistantMessageArgs,
    ChatCompletionRequestMessage as Message,
};
use serde::{Deserialize, Serialize};

/// Конфигурация запроса к LLM.
/// Сериализуемая структура для совместимости с разными платформами.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LLMRequest {
    /// Текст запроса пользователя
    pub prompt: String,
    /// Название модели (например, "gpt-3.5-turbo", "gpt-4")
    pub model: String,
    /// Системная инструкция (опционально)
    pub system_prompt: Option<String>,
    /// Максимальное количество токенов в ответе (опционально)
    pub max_completion_tokens: Option<u32>,
    /// Формат ответа: Text или JsonObject (опционально)
    #[serde(skip)]
    pub response_format: Option<ResponseFormat>,
    /// Последовательности завершения генерации (до 4, опционально)
    #[serde(skip)]
    pub stop: Option<Stop>,
    /// Температура сэмплирования (0.0–2.0). Чем выше — тем случайнее ответ.
    pub temperature: Option<f32>,
    /// Top-p (nucleus sampling, 0.0–1.0). Альтернатива temperature.
    pub top_p: Option<f32>,
    /// Штраф за повторение уже встречавшихся токенов (-2.0–2.0).
    pub frequency_penalty: Option<f32>,
    /// Штраф за упоминание новых тем (-2.0–2.0).
    pub presence_penalty: Option<f32>,
    /// Seed для воспроизводимых результатов (опционально).
    pub seed: Option<i64>,
    /// JSON Schema для структурированного ответа (строка JSON, опционально).
    /// Используется с response_format = JsonSchema.
    pub json_schema: Option<String>,
}

impl Default for LLMRequest {
    fn default() -> Self {
        Self {
            prompt: String::new(),
            model: "gpt-3.5-turbo".to_string(),
            system_prompt: None,
            max_completion_tokens: None,
            response_format: None,
            stop: None,
            temperature: None,
            top_p: None,
            frequency_penalty: None,
            presence_penalty: None,
            seed: None,
            json_schema: None,
        }
    }
}

/// Ответ от LLM.
/// Универсальная структура для легкой интеграции в любой интерфейс.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LLMResponse {
    /// Текст ответа модели
    pub content: String,
    /// Название использованной модели
    pub model: String,
    /// Количество использованных токенов
    pub usage: Option<TokenUsage>,
}

/// Информация об использовании токенов.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TokenUsage {
    pub prompt_tokens: u32,
    pub completion_tokens: u32,
    pub total_tokens: u32,
}

/// Конфигурация клиента для подключения к LLM API.
/// Поддерживает OpenAI и совместимые сервисы (LM Studio, Ollama, vLLM).
#[derive(Debug, Clone, Default)]
pub struct ClientConfig {
    /// API ключ
    pub api_key: String,
    /// Базовый URL API (по умолчанию https://api.openai.com/v1)
    pub base_url: Option<String>,
}

/// Строит API-запрос из LLMRequest. Используется в chat() и chat_stream().
fn build_api_request(request: LLMRequest) -> Result<CreateChatCompletionRequest> {
    let mut messages = Vec::new();

    if let Some(ref system_prompt) = request.system_prompt {
        let system_message = ChatCompletionRequestSystemMessageArgs::default()
            .content(system_prompt.as_str())
            .build()
            .context("Ошибка создания системного сообщения")?;
        messages.push(system_message.into());
    }

    let user_message = ChatCompletionRequestUserMessageArgs::default()
        .content(request.prompt.as_str())
        .build()
        .context("Ошибка создания пользовательского сообщения")?;
    messages.push(user_message.into());

    let mut builder = CreateChatCompletionRequestArgs::default();
    builder.model(&request.model).messages(messages);

    if let Some(n) = request.max_completion_tokens {
        builder.max_completion_tokens(n);
    }
    if let Some(fmt) = request.response_format {
        let fmt = match fmt {
            ResponseFormat::JsonSchema { json_schema: inner } => {
                if let Some(ref raw) = request.json_schema {
                    let schema_value: serde_json::Value = serde_json::from_str(raw)
                        .context("Невалидный JSON в --json-schema / LLM_JSON_SCHEMA")?;
                    ResponseFormat::JsonSchema {
                        json_schema: async_openai::types::ResponseFormatJsonSchema {
                            schema: Some(schema_value),
                            ..inner
                        },
                    }
                } else {
                    ResponseFormat::JsonSchema { json_schema: inner }
                }
            }
            other => other,
        };
        builder.response_format(fmt);
    }
    if let Some(stop) = request.stop {
        builder.stop(stop);
    }
    if let Some(t) = request.temperature {
        builder.temperature(t);
    }
    if let Some(p) = request.top_p {
        builder.top_p(p);
    }
    if let Some(fp) = request.frequency_penalty {
        builder.frequency_penalty(fp);
    }
    if let Some(pp) = request.presence_penalty {
        builder.presence_penalty(pp);
    }
    if let Some(s) = request.seed {
        builder.seed(s);
    }

    builder.build().context("Ошибка формирования запроса к OpenAI API")
}

/// Клиент для работы с OpenAI API.
/// Инкапсулирует всю бизнес-логику взаимодействия с LLM.
pub struct OpenAIClient {
    client: Client<OpenAIConfig>,
}

impl OpenAIClient {
    /// Создает нового клиента с указанной конфигурацией.
    pub fn new(config: ClientConfig) -> Self {
        let mut openai_config = OpenAIConfig::default().with_api_key(&config.api_key);
        
        if let Some(base_url) = config.base_url {
            openai_config = openai_config.with_api_base(&base_url);
        }
        
        Self {
            client: Client::with_config(openai_config),
        }
    }

    /// Создает клиента из переменных окружения.
    /// 
    /// Ожидаемые переменные:
    /// - `OPENAI_API_KEY` (обязательно)
    /// - `OPENAI_BASE_URL` (опционально, для локальных моделей)
    pub fn from_env() -> Result<Self> {
        let api_key = validate_api_key()?;
        let base_url = std::env::var("OPENAI_BASE_URL").ok();
        
        Ok(Self::new(ClientConfig { api_key, base_url }))
    }

    /// Отправляет запрос к LLM и возвращает типизированный ответ.
    /// 
    /// # Аргументы
    /// * `request` - Структура запроса с промптом и параметрами модели
    ///
    /// # Возвращает
    /// * `Result<LLMResponse>` - Ответ модели или ошибку
    ///
    /// # Пример
    /// ```no_run
    /// use ai_cli_assistant::{OpenAIClient, ClientConfig, LLMRequest};
    ///
    /// async fn example() -> anyhow::Result<()> {
    ///     let client = OpenAIClient::new(ClientConfig {
    ///         api_key: "your-api-key".to_string(),
    ///         base_url: None,
    ///     });
    ///     let request = LLMRequest {
    ///         prompt: "Объясни квантовую запутанность".to_string(),
    ///         model: "gpt-3.5-turbo".to_string(),
    ///         ..Default::default()
    ///     };
    ///     let response = client.chat(request).await?;
    ///     println!("{}", response.content);
    ///     Ok(())
    /// }
    /// ```
    pub async fn chat(&self, request: LLMRequest) -> Result<LLMResponse> {
        let api_request = build_api_request(request)?;

        let response = self
            .client
            .chat()
            .create(api_request)
            .await
            .context("Ошибка при получении ответа от OpenAI")?;

        // Извлекаем ответ
        let choice = response.choices.first()
            .ok_or_else(|| anyhow::anyhow!("Сервер не вернул вариантов ответа (choices)"))?;

        let content = choice.message.content
            .clone()
            .ok_or_else(|| anyhow::anyhow!("Получен пустой ответ от модели"))?;

        // Формируем результат с информацией о токенах
        let usage = response.usage.map(|u| TokenUsage {
            prompt_tokens: u.prompt_tokens,
            completion_tokens: u.completion_tokens,
            total_tokens: u.total_tokens,
        });

        Ok(LLMResponse {
            content,
            model: response.model,
            usage,
        })
    }

    /// Отправляет запрос к LLM в режиме стриминга.
    ///
    /// Возвращает поток чанков. Каждый чанк содержит часть генерируемого ответа.
    /// Используйте `futures::StreamExt::next()` для итерации.
    pub async fn chat_stream(&self, request: LLMRequest) -> Result<ChatCompletionResponseStream> {
        let api_request = build_api_request(request)?;

        self.client
            .chat()
            .create_stream(api_request)
            .await
            .context("Ошибка запуска стрима")
    }

    /// Стриминг с произвольной историей сообщений (для многоходового диалога).
    pub async fn chat_stream_messages(
        &self,
        model: &str,
        messages: Vec<ChatCompletionRequestMessage>,
        max_tokens: Option<u32>,
    ) -> Result<ChatCompletionResponseStream> {
        let mut builder = CreateChatCompletionRequestArgs::default();
        builder.model(model).messages(messages);
        if let Some(n) = max_tokens {
            builder.max_completion_tokens(n);
        }
        let api_request = builder.build().context("Ошибка формирования запроса")?;

        self.client
            .chat()
            .create_stream(api_request)
            .await
            .context("Ошибка запуска стрима")
    }

    /// Быстрый запрос с простым строковым ответом.
    /// Удобно для простых сценариев без необходимости работы со структурами.
    pub async fn chat_simple(&self, prompt: &str, model: &str) -> Result<String> {
        let request = LLMRequest {
            prompt: prompt.to_string(),
            model: model.to_string(),
            ..Default::default()
        };
        let response = self.chat(request).await?;
        Ok(response.content)
    }
}

/// Утилита для проверки наличия API ключа в окружении.
pub fn validate_api_key() -> Result<String> {
    std::env::var("OPENAI_API_KEY").context(
        "Переменная окружения OPENAI_API_KEY не найдена. ".to_owned() +
        "Установите её командой 'export OPENAI_API_KEY=sk-...' или создайте файл .env"
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use async_openai::types::{ResponseFormat, Stop};

    // ── Глобальный мьютекс для тестов, которые трогают env переменные ─────────
    static ENV_MUTEX: std::sync::Mutex<()> = std::sync::Mutex::new(());

    // ── LLMRequest: поля и дефолты ────────────────────────────────────────────

    #[test]
    fn test_llm_request_default() {
        let r = LLMRequest::default();
        assert_eq!(r.model, "gpt-3.5-turbo");
        assert!(r.prompt.is_empty());
        assert!(r.system_prompt.is_none());
        assert!(r.max_completion_tokens.is_none());
        assert!(r.response_format.is_none());
        assert!(r.stop.is_none());
        assert!(r.temperature.is_none());
        assert!(r.top_p.is_none());
        assert!(r.frequency_penalty.is_none());
        assert!(r.presence_penalty.is_none());
        assert!(r.seed.is_none());
        assert!(r.json_schema.is_none());
    }

    #[test]
    fn test_llm_request_custom() {
        let r = LLMRequest {
            prompt: "test".to_string(),
            model: "gpt-4".to_string(),
            system_prompt: Some("You are helpful".to_string()),
            max_completion_tokens: Some(200),
            temperature: Some(0.7),
            seed: Some(42),
            json_schema: Some(r#"{"type":"object"}"#.to_string()),
            ..Default::default()
        };
        assert_eq!(r.prompt, "test");
        assert_eq!(r.model, "gpt-4");
        assert_eq!(r.system_prompt.as_deref(), Some("You are helpful"));
        assert_eq!(r.max_completion_tokens, Some(200));
        assert_eq!(r.temperature, Some(0.7));
        assert_eq!(r.seed, Some(42));
        assert!(r.json_schema.is_some());
    }

    // ── LLMRequest: сериализация ──────────────────────────────────────────────

    #[test]
    fn test_llm_request_serde_skips_response_format_and_stop() {
        let r = LLMRequest {
            prompt: "hi".to_string(),
            model: "gpt-4o".to_string(),
            response_format: Some(ResponseFormat::Text),
            stop: Some(Stop::String("END".to_string())),
            ..Default::default()
        };
        let json = serde_json::to_string(&r).unwrap();
        // Поля помечены #[serde(skip)] — не должны сериализоваться
        assert!(!json.contains("response_format"));
        assert!(!json.contains("\"stop\""));
    }

    #[test]
    fn test_llm_request_serde_json_schema_field_included() {
        let r = LLMRequest {
            prompt: "hi".to_string(),
            model: "gpt-4o".to_string(),
            json_schema: Some(r#"{"type":"object"}"#.to_string()),
            ..Default::default()
        };
        let json = serde_json::to_string(&r).unwrap();
        assert!(json.contains("json_schema"));
        // Десериализация восстанавливает поле
        let r2: LLMRequest = serde_json::from_str(&json).unwrap();
        assert_eq!(r2.json_schema, r.json_schema);
    }

    #[test]
    fn test_llm_request_serde_roundtrip() {
        let r = LLMRequest {
            prompt: "hello".to_string(),
            model: "gpt-4o".to_string(),
            system_prompt: Some("Be concise".to_string()),
            max_completion_tokens: Some(100),
            temperature: Some(0.5),
            top_p: Some(0.9),
            frequency_penalty: Some(0.1),
            presence_penalty: Some(-0.1),
            seed: Some(7),
            json_schema: Some(r#"{"type":"string"}"#.to_string()),
            ..Default::default()
        };
        let json = serde_json::to_string(&r).unwrap();
        let r2: LLMRequest = serde_json::from_str(&json).unwrap();
        assert_eq!(r.prompt, r2.prompt);
        assert_eq!(r.model, r2.model);
        assert_eq!(r.system_prompt, r2.system_prompt);
        assert_eq!(r.max_completion_tokens, r2.max_completion_tokens);
        assert_eq!(r.temperature, r2.temperature);
        assert_eq!(r.seed, r2.seed);
        assert_eq!(r.json_schema, r2.json_schema);
        // response_format и stop не сериализуются — после roundtrip None
        assert!(r2.response_format.is_none());
        assert!(r2.stop.is_none());
    }

    // ── build_api_request ─────────────────────────────────────────────────────

    #[test]
    fn test_build_api_request_minimal() {
        let r = LLMRequest {
            prompt: "hello".to_string(),
            model: "gpt-4o".to_string(),
            ..Default::default()
        };
        let req = build_api_request(r).unwrap();
        assert_eq!(req.model, "gpt-4o");
        assert_eq!(req.messages.len(), 1); // только user message
        assert!(req.max_completion_tokens.is_none());
        assert!(req.temperature.is_none());
        assert!(req.response_format.is_none());
    }

    #[test]
    fn test_build_api_request_with_system_prompt() {
        let r = LLMRequest {
            prompt: "hello".to_string(),
            model: "gpt-4o".to_string(),
            system_prompt: Some("You are a Rust expert".to_string()),
            ..Default::default()
        };
        let req = build_api_request(r).unwrap();
        // system + user = 2 сообщения
        assert_eq!(req.messages.len(), 2);
    }

    #[test]
    fn test_build_api_request_without_system_prompt() {
        let r = LLMRequest {
            prompt: "hello".to_string(),
            model: "gpt-4o".to_string(),
            system_prompt: None,
            ..Default::default()
        };
        let req = build_api_request(r).unwrap();
        assert_eq!(req.messages.len(), 1);
    }

    #[test]
    fn test_build_api_request_max_tokens() {
        let r = LLMRequest {
            prompt: "hi".to_string(),
            model: "gpt-4o".to_string(),
            max_completion_tokens: Some(256),
            ..Default::default()
        };
        let req = build_api_request(r).unwrap();
        assert_eq!(req.max_completion_tokens, Some(256));
    }

    #[test]
    fn test_build_api_request_temperature_and_seed() {
        let r = LLMRequest {
            prompt: "hi".to_string(),
            model: "gpt-4o".to_string(),
            temperature: Some(0.3),
            seed: Some(123),
            ..Default::default()
        };
        let req = build_api_request(r).unwrap();
        assert_eq!(req.temperature, Some(0.3));
        assert_eq!(req.seed, Some(123));
    }

    #[test]
    fn test_build_api_request_sampling_params() {
        let r = LLMRequest {
            prompt: "hi".to_string(),
            model: "gpt-4o".to_string(),
            top_p: Some(0.8),
            frequency_penalty: Some(0.5),
            presence_penalty: Some(-0.3),
            ..Default::default()
        };
        let req = build_api_request(r).unwrap();
        assert_eq!(req.top_p, Some(0.8));
        assert_eq!(req.frequency_penalty, Some(0.5));
        assert_eq!(req.presence_penalty, Some(-0.3));
    }

    #[test]
    fn test_build_api_request_response_format_text() {
        let r = LLMRequest {
            prompt: "hi".to_string(),
            model: "gpt-4o".to_string(),
            response_format: Some(ResponseFormat::Text),
            ..Default::default()
        };
        let req = build_api_request(r).unwrap();
        assert!(matches!(req.response_format, Some(ResponseFormat::Text)));
    }

    #[test]
    fn test_build_api_request_json_schema_without_body() {
        // --response-format json-schema без --json-schema → schema остаётся None
        let r = LLMRequest {
            prompt: "hi".to_string(),
            model: "gpt-4o".to_string(),
            response_format: Some(ResponseFormat::JsonSchema {
                json_schema: async_openai::types::ResponseFormatJsonSchema {
                    name: "response".to_string(),
                    strict: Some(false),
                    description: None,
                    schema: None,
                },
            }),
            json_schema: None,
            ..Default::default()
        };
        let req = build_api_request(r).unwrap();
        if let Some(ResponseFormat::JsonSchema { json_schema }) = req.response_format {
            assert!(json_schema.schema.is_none());
            assert_eq!(json_schema.name, "response");
        } else {
            panic!("expected JsonSchema response_format");
        }
    }

    #[test]
    fn test_build_api_request_json_schema_with_valid_body() {
        let schema_str = r#"{"type":"object","properties":{"answer":{"type":"string"}},"required":["answer"],"additionalProperties":false}"#;
        let r = LLMRequest {
            prompt: "hi".to_string(),
            model: "gpt-4o".to_string(),
            response_format: Some(ResponseFormat::JsonSchema {
                json_schema: async_openai::types::ResponseFormatJsonSchema {
                    name: "response".to_string(),
                    strict: Some(false),
                    description: None,
                    schema: None,
                },
            }),
            json_schema: Some(schema_str.to_string()),
            ..Default::default()
        };
        let req = build_api_request(r).unwrap();
        if let Some(ResponseFormat::JsonSchema { json_schema }) = req.response_format {
            let schema = json_schema.schema.expect("schema должна быть Some");
            assert_eq!(schema["type"], "object");
            assert!(schema["properties"]["answer"].is_object());
            assert_eq!(schema["required"][0], "answer");
        } else {
            panic!("expected JsonSchema response_format");
        }
    }

    #[test]
    fn test_build_api_request_json_schema_invalid_json_returns_error() {
        let r = LLMRequest {
            prompt: "hi".to_string(),
            model: "gpt-4o".to_string(),
            response_format: Some(ResponseFormat::JsonSchema {
                json_schema: async_openai::types::ResponseFormatJsonSchema {
                    name: "response".to_string(),
                    strict: Some(false),
                    description: None,
                    schema: None,
                },
            }),
            json_schema: Some("not valid json {{{".to_string()),
            ..Default::default()
        };
        let result = build_api_request(r);
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("Невалидный JSON"));
    }

    #[test]
    fn test_build_api_request_text_format_ignores_json_schema_field() {
        // Если format = Text, поле json_schema не должно влиять на результат
        let r = LLMRequest {
            prompt: "hi".to_string(),
            model: "gpt-4o".to_string(),
            response_format: Some(ResponseFormat::Text),
            json_schema: Some(r#"{"type":"object"}"#.to_string()),
            ..Default::default()
        };
        let req = build_api_request(r).unwrap();
        assert!(matches!(req.response_format, Some(ResponseFormat::Text)));
    }

    #[test]
    fn test_build_api_request_stop_single_string() {
        let r = LLMRequest {
            prompt: "hi".to_string(),
            model: "gpt-4o".to_string(),
            stop: Some(Stop::String("END".to_string())),
            ..Default::default()
        };
        let req = build_api_request(r).unwrap();
        assert!(matches!(req.stop, Some(Stop::String(ref s)) if s == "END"));
    }

    #[test]
    fn test_build_api_request_stop_string_array() {
        let r = LLMRequest {
            prompt: "hi".to_string(),
            model: "gpt-4o".to_string(),
            stop: Some(Stop::StringArray(vec!["END".to_string(), "###".to_string()])),
            ..Default::default()
        };
        let req = build_api_request(r).unwrap();
        if let Some(Stop::StringArray(arr)) = req.stop {
            assert_eq!(arr.len(), 2);
            assert_eq!(arr[0], "END");
            assert_eq!(arr[1], "###");
        } else {
            panic!("expected StringArray stop");
        }
    }

    // ── ClientConfig ──────────────────────────────────────────────────────────

    #[test]
    fn test_client_config_default() {
        let config = ClientConfig::default();
        assert!(config.api_key.is_empty());
        assert!(config.base_url.is_none());
    }

    #[test]
    fn test_client_config_custom() {
        let config = ClientConfig {
            api_key: "test-key".to_string(),
            base_url: Some("http://localhost:1234/v1".to_string()),
        };
        assert_eq!(config.api_key, "test-key");
        assert_eq!(config.base_url, Some("http://localhost:1234/v1".to_string()));
    }

    #[test]
    fn test_client_creation_does_not_panic() {
        let config = ClientConfig {
            api_key: "test-key".to_string(),
            base_url: Some("http://localhost:1234/v1".to_string()),
        };
        let _client = OpenAIClient::new(config);
    }

    #[test]
    fn test_client_creation_without_base_url() {
        let config = ClientConfig {
            api_key: "test-key".to_string(),
            base_url: None,
        };
        let _client = OpenAIClient::new(config);
    }

    // ── validate_api_key ──────────────────────────────────────────────────────

    #[test]
    fn test_validate_api_key_present() {
        let _lock = ENV_MUTEX.lock().unwrap();
        // SAFETY: тест держит мьютекс — другие тесты, трогающие env, не выполняются параллельно
        unsafe { std::env::set_var("OPENAI_API_KEY", "sk-test-key"); }
        let result = validate_api_key();
        unsafe { std::env::remove_var("OPENAI_API_KEY"); }
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), "sk-test-key");
    }

    #[test]
    fn test_validate_api_key_missing_returns_error() {
        let _lock = ENV_MUTEX.lock().unwrap();
        let prev = std::env::var("OPENAI_API_KEY").ok();
        // SAFETY: тест держит мьютекс — другие тесты, трогающие env, не выполняются параллельно
        unsafe { std::env::remove_var("OPENAI_API_KEY"); }
        let result = validate_api_key();
        if let Some(val) = prev {
            unsafe { std::env::set_var("OPENAI_API_KEY", val); }
        }
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("OPENAI_API_KEY"));
    }
}
