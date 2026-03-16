//! CLI обертка для библиотеки ai_cli_assistant.
//! 
//! Этот модуль отвечает только за взаимодей с пользователем через
//! командную строку. Вся бизнес-логика делегируется библиотеке.

use ai_cli_assistant::OpenAIClient;
use anyhow::Result;
use clap::Parser;
use dotenv::dotenv;
use indicatif::{ProgressBar, ProgressStyle};
use std::time::Duration;

/// Аргументы командной строки для AI CLI Assistant.
#[derive(Parser, Debug)]
#[command(
    author, 
    version, 
    about = "Кроссплатформенный CLI клиент для работы с LLM (OpenAI, LM Studio, Ollama)",
    long_about = "AI CLI Assistant - это консольный инструмент для взаимодействия \
                  с языковыми моделями. Поддерживает OpenAI API и совместимые сервисы \
                  (LM Studio, Ollama, vLLM). Все настройки через ENV переменные."
)]
struct Args {
    /// Текст запроса к нейросети
    #[arg(short, long)]
    prompt: String,

    /// Название модели (по умолчанию gpt-3.5-turbo)
    #[arg(short, long, default_value = "gpt-3.5-turbo")]
    model: String,

    /// Системная инструкция для модели (опционально)
    #[arg(short, long)]
    system: Option<String>,

    /// Уровень детализации (для отладки)
    #[arg(short, long, default_value = "false")]
    verbose: bool,

    /// Показать информацию об использовании токенов
    #[arg(long, default_value = "false")]
    show_usage: bool,
}

/// Точка входа в CLI приложение.
#[tokio::main]
async fn main() -> Result<()> {
    // Инициализация окружения
    dotenv().ok();

    // Парсинг аргументов командной строки
    let args = Args::parse();

    if args.verbose {
        eprintln!("[DEBUG] Запуск AI CLI Assistant v{}", env!("CARGO_PKG_VERSION"));
        eprintln!("[DEBUG] Модель: {}", args.model);
        eprintln!("[DEBUG] Запрос: {}", args.prompt);
        if let Some(ref sys) = args.system {
            eprintln!("[DEBUG] Системный промпт: {}", sys);
        }
        // Показываем конфигурацию подключения
        if let Ok(_) = std::env::var("OPENAI_API_KEY") {
            eprintln!("[DEBUG] OPENAI_API_KEY: установлен");
        }
        if let Ok(base_url) = std::env::var("OPENAI_BASE_URL") {
            eprintln!("[DEBUG] OPENAI_BASE_URL: {}", base_url);
        } else {
            eprintln!("[DEBUG] OPENAI_BASE_URL: не установлен (используется OpenAI по умолчанию)");
        }
    }

    // Инициализация клиента из ENV переменных
    let client = OpenAIClient::from_env()?;

    // Формирование запроса
    let request = ai_cli_assistant::LLMRequest {
        prompt: args.prompt,
        model: args.model,
        system_prompt: args.system,
    };

    if args.verbose {
        eprintln!("[DEBUG] Отправка запроса к серверу...");
    }

    // Создаём и запускаем спиннер
    let spinner = ProgressBar::new_spinner();
    spinner.set_style(
        ProgressStyle::default_spinner()
            .tick_chars("⠋⠙⠹⠸⠼⠴⠦⠧⠇⠏")
            .template("{spinner} {msg}")
            .unwrap()
    );
    spinner.set_message("Генерация ответа...");
    spinner.enable_steady_tick(Duration::from_millis(80));

    // Получение ответа
    let response = client.chat(request).await?;

    // Останавливаем спиннер
    spinner.finish_and_clear();

    // Вывод результата
    println!("\n{}", response.content);

    // Опционально: информация об использовании токенов
    if args.show_usage {
        if let Some(usage) = &response.usage {
            eprintln!("\n--- Использование токенов ---");
            eprintln!("Prompt tokens:      {}", usage.prompt_tokens);
            eprintln!("Completion tokens:  {}", usage.completion_tokens);
            eprintln!("Total tokens:       {}", usage.total_tokens);
        }
    }

    Ok(())
}
