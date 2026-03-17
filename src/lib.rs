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
        ChatCompletionRequestSystemMessageArgs,
        ChatCompletionRequestUserMessageArgs,
        CreateChatCompletionRequestArgs,
    },
    Client,
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
}

impl Default for LLMRequest {
    fn default() -> Self {
        Self {
            prompt: String::new(),
            model: "gpt-3.5-turbo".to_string(),
            system_prompt: None,
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
    ///         system_prompt: None,
    ///     };
    ///     let response = client.chat(request).await?;
    ///     println!("{}", response.content);
    ///     Ok(())
    /// }
    /// ```
    pub async fn chat(&self, request: LLMRequest) -> Result<LLMResponse> {
        // Формируем сообщения для чата
        let mut messages = Vec::new();

        // Добавляем системное сообщение, если указано
        if let Some(ref system_prompt) = request.system_prompt {
            let system_message = ChatCompletionRequestSystemMessageArgs::default()
                .content(system_prompt.as_str())
                .build()
                .context("Ошибка создания системного сообщения")?;
            messages.push(system_message.into());
        }

        // Добавляем пользовательское сообщение
        let user_message = ChatCompletionRequestUserMessageArgs::default()
            .content(request.prompt.as_str())
            .build()
            .context("Ошибка создания пользовательского сообщения")?;
        messages.push(user_message.into());

        // Создаем запрос к API
        let api_request = CreateChatCompletionRequestArgs::default()
            .model(&request.model)
            .messages(messages)
            .build()
            .context("Ошибка формирования запроса к OpenAI API")?;

        // Отправляем запрос
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

    /// Быстрый запрос с простым строковым ответом.
    /// Удобно для простых сценариев без необходимости работы со структурами.
    pub async fn chat_simple(&self, prompt: &str, model: &str) -> Result<String> {
        let request = LLMRequest {
            prompt: prompt.to_string(),
            model: model.to_string(),
            system_prompt: None,
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

    #[test]
    fn test_llm_request_default() {
        let request = LLMRequest::default();
        assert_eq!(request.model, "gpt-3.5-turbo");
        assert!(request.prompt.is_empty());
        assert!(request.system_prompt.is_none());
    }

    #[test]
    fn test_llm_request_custom() {
        let request = LLMRequest {
            prompt: "test".to_string(),
            model: "gpt-4".to_string(),
            system_prompt: Some("You are helpful".to_string()),
        };
        assert_eq!(request.prompt, "test");
        assert_eq!(request.model, "gpt-4");
        assert!(request.system_prompt.is_some());
    }

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
    fn test_client_creation() {
        let config = ClientConfig {
            api_key: "test-key".to_string(),
            base_url: Some("http://localhost:1234/v1".to_string()),
        };
        let _client = OpenAIClient::new(config);
        // Тест просто проверяет, что клиент создается без паники
    }
}
