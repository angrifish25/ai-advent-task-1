//! CLI обертка для библиотеки ai_cli_assistant.
//!
//! Этот модуль отвечает только за взаимодействие с пользователем через
//! командную строку. Вся бизнес-логика делегируется библиотеке.

use ai_cli_assistant::{LLMRequest, Message, OpenAIClient};
use anyhow::Result;
use async_openai::types::{
    ChatCompletionRequestAssistantMessageArgs, ChatCompletionRequestSystemMessageArgs,
    ChatCompletionRequestUserMessageArgs, ResponseFormat, ResponseFormatJsonSchema, Stop,
};
use clap::{Args, Parser, Subcommand};
use console::Term;
use dotenvy::dotenv;
use futures::StreamExt;
use indicatif::{ProgressBar, ProgressStyle};
use std::time::Duration;

/// Кроссплатформенный CLI клиент для работы с LLM (OpenAI, LM Studio, Ollama).
#[derive(Parser, Debug)]
#[command(
    author,
    version,
    about = "Кроссплатформенный CLI клиент для работы с LLM (OpenAI, LM Studio, Ollama)",
    long_about = "AI CLI Assistant - консольный инструмент для взаимодействия \
                  с языковыми моделями. Поддерживает OpenAI API и совместимые сервисы \
                  (LM Studio, Ollama, vLLM). Настройки подключения — через ENV переменные."
)]
struct Cli {
    /// Режим отладки: показывает конфигурацию и детали запроса
    #[arg(short, long, global = true)]
    verbose: bool,

    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand, Debug)]
enum Command {
    /// Одиночный запрос — получить ответ и выйти
    Ask(AskArgs),
    /// Стриминг ответа токен за токеном (прервать — любой клавишей)
    Stream(CommonArgs),
    /// Интерактивный диалог с сохранением истории (выход — пустая строка)
    Chat(CommonArgs),
}

/// Параметры, общие для всех режимов.
#[derive(Args, Debug)]
struct CommonArgs {
    /// Текст запроса к модели
    #[arg(short, long)]
    prompt: String,

    /// Название модели (приоритет: CLI > OPENAI_DEFAULT_MODEL > gpt-3.5-turbo)
    #[arg(short, long)]
    model: Option<String>,

    /// Системная инструкция (приоритет: CLI > LLM_SYSTEM_PROMPT)
    #[arg(short, long)]
    system: Option<String>,

    /// Максимальное количество токенов в ответе (приоритет: CLI > LLM_MAX_TOKENS)
    #[arg(long)]
    max_tokens: Option<u32>,

    /// Формат ответа: text | json-schema
    #[arg(long, value_enum)]
    response_format: Option<ResponseFormatArg>,

    /// Stop-последовательность, можно указать до 4 раз
    #[arg(long)]
    stop: Vec<String>,

    /// Температура сэмплирования 0.0–2.0 (приоритет: CLI > LLM_TEMPERATURE)
    #[arg(long)]
    temperature: Option<f32>,

    /// Nucleus sampling 0.0–1.0 (приоритет: CLI > LLM_TOP_P)
    #[arg(long)]
    top_p: Option<f32>,

    /// Штраф за повторяющиеся токены -2.0–2.0 (приоритет: CLI > LLM_FREQUENCY_PENALTY)
    #[arg(long)]
    frequency_penalty: Option<f32>,

    /// Штраф за повторяющиеся темы -2.0–2.0 (приоритет: CLI > LLM_PRESENCE_PENALTY)
    #[arg(long)]
    presence_penalty: Option<f32>,

    /// Seed для воспроизводимых ответов (приоритет: CLI > LLM_SEED)
    #[arg(long)]
    seed: Option<i64>,

    /// JSON Schema для структурированного ответа (строка JSON). Используется с --response-format json-schema
    #[arg(long)]
    json_schema: Option<String>,
}

/// Аргументы субкоманды `ask` — включают общие параметры и --show-usage.
#[derive(Args, Debug)]
struct AskArgs {
    #[command(flatten)]
    common: CommonArgs,

    /// Показать статистику использования токенов
    #[arg(long)]
    show_usage: bool,
}

/// Формат ответа для флага --response-format.
#[derive(clap::ValueEnum, Clone, Debug)]
enum ResponseFormatArg {
    /// Обычный текст (по умолчанию)
    Text,
    /// Структурированный JSON по схеме (gpt-4o и новее)
    JsonSchema,
}

#[tokio::main]
async fn main() -> Result<()> {
    dotenv().ok();

    let cli = Cli::parse();
    let verbose = cli.verbose;

    // Вспомогательные замыкания для чтения числовых ENV-переменных
    let env_f32 = |key: &str| -> Option<f32> { std::env::var(key).ok()?.parse().ok() };
    let env_u32 = |key: &str| -> Option<u32> { std::env::var(key).ok()?.parse().ok() };
    let env_i64 = |key: &str| -> Option<i64> { std::env::var(key).ok()?.parse().ok() };

    let client = OpenAIClient::from_env()?;

    match cli.command {
        Command::Ask(args) => {
            let request = build_request(args.common, &env_f32, &env_u32, &env_i64);
            if verbose { print_debug(&request); }
            run_blocking_mode(client, request, args.show_usage).await?;
        }
        Command::Stream(args) => {
            let request = build_request(args, &env_f32, &env_u32, &env_i64);
            if verbose { print_debug(&request); }
            run_stream_mode(client, request).await?;
        }
        Command::Chat(args) => {
            let request = build_request(args, &env_f32, &env_u32, &env_i64);
            if verbose { print_debug(&request); }
            run_chat_mode(client, request).await?;
        }
    }

    Ok(())
}

/// Собирает LLMRequest из CommonArgs.
/// Приоритет для каждого параметра: CLI > ENV (рантайм) > вшитый дефолт (из .env на момент сборки).
fn build_request(
    args: CommonArgs,
    env_f32: &dyn Fn(&str) -> Option<f32>,
    env_u32: &dyn Fn(&str) -> Option<u32>,
    env_i64: &dyn Fn(&str) -> Option<i64>,
) -> LLMRequest {
    // Вспомогательные парсеры вшитых дефолтов
    let build_f32 = |key: &str| -> Option<f32> { option_env_str(key)?.parse().ok() };
    let build_u32 = |key: &str| -> Option<u32> { option_env_str(key)?.parse().ok() };
    let build_i64 = |key: &str| -> Option<i64> { option_env_str(key)?.parse().ok() };

    let model = args.model
        .or_else(|| std::env::var("OPENAI_DEFAULT_MODEL").ok())
        .or_else(|| option_env_str("BUILD_DEFAULT_OPENAI_DEFAULT_MODEL").map(str::to_owned))
        .unwrap_or_else(|| "gpt-3.5-turbo".to_string());

    let response_format = args.response_format.map(|f| match f {
        ResponseFormatArg::Text => ResponseFormat::Text,
        ResponseFormatArg::JsonSchema => ResponseFormat::JsonSchema {
            json_schema: ResponseFormatJsonSchema {
                name: "response".to_string(),
                strict: Some(false),
                description: None,
                schema: None,
            },
        },
    });

    let stop = match args.stop.len() {
        0 => None,
        1 => Some(Stop::String(args.stop.into_iter().next().unwrap())),
        _ => Some(Stop::StringArray(args.stop)),
    };

    LLMRequest {
        prompt: args.prompt,
        model,
        system_prompt: args.system
            .or_else(|| std::env::var("LLM_SYSTEM_PROMPT").ok())
            .or_else(|| option_env_str("BUILD_DEFAULT_LLM_SYSTEM_PROMPT").map(str::to_owned)),
        max_completion_tokens: args.max_tokens
            .or_else(|| env_u32("LLM_MAX_TOKENS"))
            .or_else(|| build_u32("BUILD_DEFAULT_LLM_MAX_TOKENS")),
        response_format,
        stop,
        temperature: args.temperature
            .or_else(|| env_f32("LLM_TEMPERATURE"))
            .or_else(|| build_f32("BUILD_DEFAULT_LLM_TEMPERATURE")),
        top_p: args.top_p
            .or_else(|| env_f32("LLM_TOP_P"))
            .or_else(|| build_f32("BUILD_DEFAULT_LLM_TOP_P")),
        frequency_penalty: args.frequency_penalty
            .or_else(|| env_f32("LLM_FREQUENCY_PENALTY"))
            .or_else(|| build_f32("BUILD_DEFAULT_LLM_FREQUENCY_PENALTY")),
        presence_penalty: args.presence_penalty
            .or_else(|| env_f32("LLM_PRESENCE_PENALTY"))
            .or_else(|| build_f32("BUILD_DEFAULT_LLM_PRESENCE_PENALTY")),
        seed: args.seed
            .or_else(|| env_i64("LLM_SEED"))
            .or_else(|| build_i64("BUILD_DEFAULT_LLM_SEED")),
        json_schema: args.json_schema
            .or_else(|| std::env::var("LLM_JSON_SCHEMA").ok())
            .or_else(|| option_env_str("BUILD_DEFAULT_LLM_JSON_SCHEMA").map(str::to_owned)),
    }
}

/// Читает compile-time константу по имени.
/// Обёртка над option_env!, которая возвращает Option<&'static str>.
fn option_env_str(key: &str) -> Option<&'static str> {
    macro_rules! check {
        ($k:literal) => {
            if key == $k { return option_env!($k); }
        };
    }
    check!("BUILD_DEFAULT_OPENAI_DEFAULT_MODEL");
    check!("BUILD_DEFAULT_LLM_SYSTEM_PROMPT");
    check!("BUILD_DEFAULT_LLM_MAX_TOKENS");
    check!("BUILD_DEFAULT_LLM_TEMPERATURE");
    check!("BUILD_DEFAULT_LLM_TOP_P");
    check!("BUILD_DEFAULT_LLM_FREQUENCY_PENALTY");
    check!("BUILD_DEFAULT_LLM_PRESENCE_PENALTY");
    check!("BUILD_DEFAULT_LLM_SEED");
    check!("BUILD_DEFAULT_LLM_JSON_SCHEMA");
    None
}

fn print_debug(request: &LLMRequest) {
    eprintln!("[DEBUG] Запуск AI CLI Assistant v{}", env!("CARGO_PKG_VERSION"));
    eprintln!("[DEBUG] Модель: {}", request.model);
    eprintln!("[DEBUG] Запрос: {}", request.prompt);
    if let Some(ref sys) = request.system_prompt {
        eprintln!("[DEBUG] Системный промпт: {}", sys);
    }
    if let Some(t) = request.temperature {
        eprintln!("[DEBUG] Temperature: {}", t);
    }
    if std::env::var("OPENAI_API_KEY").is_ok() {
        eprintln!("[DEBUG] OPENAI_API_KEY: установлен");
    }
    match std::env::var("OPENAI_BASE_URL") {
        Ok(url) => eprintln!("[DEBUG] OPENAI_BASE_URL: {}", url),
        Err(_) => eprintln!("[DEBUG] OPENAI_BASE_URL: не установлен (OpenAI cloud)"),
    }
}

fn make_spinner(msg: &'static str) -> ProgressBar {
    let spinner = ProgressBar::new_spinner();
    spinner.set_style(
        ProgressStyle::default_spinner()
            .tick_chars("⠋⠙⠹⠸⠼⠴⠦⠧⠇⠏")
            .template("{spinner} {msg}")
            .unwrap(),
    );
    spinner.set_message(msg);
    spinner.enable_steady_tick(Duration::from_millis(80));
    spinner
}

async fn run_blocking_mode(client: OpenAIClient, request: LLMRequest, show_usage: bool) -> Result<()> {
    let spinner = make_spinner("Генерация ответа...");
    let response = client.chat(request).await?;
    spinner.finish_and_clear();

    println!("\n{}", response.content);

    if show_usage {
        if let Some(usage) = &response.usage {
            eprintln!("\n--- Использование токенов ---");
            eprintln!("Prompt tokens:      {}", usage.prompt_tokens);
            eprintln!("Completion tokens:  {}", usage.completion_tokens);
            eprintln!("Total tokens:       {}", usage.total_tokens);
        }
    }

    Ok(())
}

async fn run_stream_mode(client: OpenAIClient, request: LLMRequest) -> Result<()> {
    use std::io::Write;
    use std::sync::mpsc;
    use std::thread;

    let spinner = make_spinner("Подключение...");
    let mut stream = client.chat_stream(request).await?;
    spinner.finish_and_clear();

    let (tx, rx) = mpsc::channel::<()>();
    thread::spawn(move || {
        let _ = Term::stderr().read_key();
        let _ = tx.send(());
    });

    eprintln!("──────────────────────────────────────");
    eprintln!("[Любая клавиша] — остановить генерацию");
    eprintln!("──────────────────────────────────────");

    let mut interrupted = false;
    while let Some(chunk) = stream.next().await {
        if rx.try_recv().is_ok() {
            interrupted = true;
            break;
        }
        if let Some(delta) = chunk?
            .choices
            .first()
            .and_then(|c| c.delta.content.as_deref())
        {
            print!("{delta}");
            std::io::stdout().flush().ok();
        }
    }

    if interrupted {
        eprintln!("\n──────────────────────────────────────");
        eprintln!("[Генерация прервана]");
    } else {
        println!();
    }

    Ok(())
}

async fn run_chat_mode(client: OpenAIClient, request: LLMRequest) -> Result<()> {
    use std::io::{self, BufRead, Write};

    let model = request.model.clone();
    let max_tokens = request.max_completion_tokens;

    let mut messages: Vec<Message> = Vec::new();

    if let Some(ref system_prompt) = request.system_prompt {
        let msg = ChatCompletionRequestSystemMessageArgs::default()
            .content(system_prompt.as_str())
            .build()?;
        messages.push(msg.into());
    }

    let first_msg = ChatCompletionRequestUserMessageArgs::default()
        .content(request.prompt.as_str())
        .build()?;
    messages.push(first_msg.into());

    eprintln!("──────────────────────────────────────");
    eprintln!("Режим диалога. Пустая строка — выход.");
    eprintln!("──────────────────────────────────────");

    loop {
        let spinner = make_spinner("Генерация ответа...");
        let mut stream = client
            .chat_stream_messages(&model, messages.clone(), max_tokens)
            .await?;

        let mut assistant_response = String::new();
        let mut first_token = true;
        while let Some(chunk) = stream.next().await {
            if let Some(delta) = chunk?
                .choices
                .first()
                .and_then(|c| c.delta.content.as_deref())
            {
                if first_token {
                    spinner.finish_and_clear();
                    first_token = false;
                }
                assistant_response.push_str(delta);
                print!("{delta}");
                io::stdout().flush().ok();
            }
        }
        if first_token {
            spinner.finish_and_clear();
        }
        println!();

        if !assistant_response.is_empty() {
            let assistant_msg = ChatCompletionRequestAssistantMessageArgs::default()
                .content(assistant_response)
                .build()?;
            messages.push(assistant_msg.into());
        }

        eprint!("\nВы: ");
        io::stderr().flush().ok();

        let mut input = String::new();
        io::stdin().lock().read_line(&mut input)?;
        let input = input.trim().to_string();

        if input.is_empty() {
            eprintln!("Диалог завершён.");
            break;
        }

        let user_msg = ChatCompletionRequestUserMessageArgs::default()
            .content(input.as_str())
            .build()?;
        messages.push(user_msg.into());
    }

    Ok(())
}
