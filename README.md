# AI CLI Assistant

Кроссплатформенный CLI клиент для работы с языковыми моделями OpenAI и совместимыми сервисами на Rust.

## Особенности

- **Разделение логики**: Бизнес-логика вынесена в отдельную библиотеку (`lib.rs`)
- **Переиспользуемость**: Библиотеку можно использовать в WASM, мобильных приложениях или бэкенде
- **Поддержка локальных моделей**: LM Studio, Ollama, vLLM через кастомный базовый URL
- **Безопасность**: API ключи загружаются из переменных окружения
- **Асинхронность**: Неблокирующий ввод-вывод через Tokio
- **Типобезопасность**: Строгая типизация запросов и ответов
- **Индикатор прогресса**: Спиннер во время ожидания ответа

## Требования

- Rust 1.70+ (установить через [rustup](https://rustup.rs/))
- API ключ OpenAI **или** локальный сервер (LM Studio, Ollama)

## Установка

1. Клонируйте репозиторий:
```bash
git clone <repository-url>
cd ai_advent
```

2. Создайте файл `.env` с конфигурацией:
```bash
cp .env.example .env
```

3. Настройте переменные окружения (см. ниже)

4. Соберите проект:
```bash
cargo build --release
```

## Конфигурация

Все настройки задаются через переменные окружения в файле `.env`:

| Переменная | Обязательная | Описание |
|------------|--------------|----------|
| `OPENAI_API_KEY` | ✅ | API ключ (для локальных моделей — любое значение) |
| `OPENAI_BASE_URL` | ❌ | Базовый URL API (по умолчанию — OpenAI cloud) |
| `OPENAI_DEFAULT_MODEL` | ❌ | Модель по умолчанию (переопределяется флагом `--model`) |

### Примеры конфигурации

#### OpenAI (cloud)
```bash
OPENAI_API_KEY=sk-...
OPENAI_DEFAULT_MODEL=gpt-4o
```

#### LM Studio (локально)
```bash
OPENAI_API_KEY=not-needed
OPENAI_BASE_URL=http://localhost:1234/v1
```

#### Ollama (локально)
```bash
OPENAI_API_KEY=ollama
OPENAI_BASE_URL=http://localhost:11434/v1
```

## Использование

### Справка по аргументам
```bash
./target/release/ai-cli-assistant --help
```

### Все доступные аргументы

| Флаг | Короткий | Описание |
|------|----------|----------|
| `--prompt` | `-p` | Текст запроса (**обязательный**) |
| `--model` | `-m` | Название модели (по умолчанию: `gpt-3.5-turbo` или `OPENAI_DEFAULT_MODEL`) |
| `--system` | `-s` | Системная инструкция |
| `--max-tokens` | | Ограничение длины ответа в токенах |
| `--response-format` | | Формат ответа: `text` или `json` |
| `--stop` | | Stop-последовательность (можно указать до 4 раз) |
| `--show-usage` | | Показать статистику токенов |
| `--verbose` | `-v` | Режим отладки |

---

### Базовый запрос
```bash
./target/release/ai-cli-assistant --prompt "Привет, как дела?"
```

### С выбором модели
```bash
./target/release/ai-cli-assistant \
  --prompt "Напиши функцию на Python" \
  --model "gpt-4o"
```

### С системным промптом
```bash
./target/release/ai-cli-assistant \
  --prompt "Переведи: Hello, world!" \
  --system "Ты профессиональный переводчик. Переводи только текст, без объяснений."
```

### Ограничение длины ответа
```bash
./target/release/ai-cli-assistant \
  --prompt "Расскажи про Rust" \
  --max-tokens 200
```

### Получить ответ в формате JSON
```bash
./target/release/ai-cli-assistant \
  --prompt "Верни список из 5 языков программирования в виде JSON-массива" \
  --response-format json
```

### Stop-последовательности
```bash
# Остановить генерацию при встрече маркера
./target/release/ai-cli-assistant \
  --prompt "Напиши инструкцию по пунктам" \
  --stop "###"

# Несколько stop-последовательностей (до 4)
./target/release/ai-cli-assistant \
  --prompt "Напиши текст" \
  --stop "###" \
  --stop "END" \
  --stop "---"
```

### Показать использование токенов
```bash
./target/release/ai-cli-assistant \
  --prompt "Объясни квантовую физику" \
  --show-usage
```

### Режим отладки
```bash
./target/release/ai-cli-assistant \
  --prompt "Тест" \
  --verbose
```

### С локальной моделью (LM Studio)
1. Запустите LM Studio и загрузите модель
2. Запустите Local Server в LM Studio
3. Настройте `.env`:
   ```bash
   OPENAI_API_KEY=not-needed
   OPENAI_BASE_URL=http://localhost:1234/v1
   ```
4. Запустите CLI:
   ```bash
   ./target/release/ai-cli-assistant \
     --prompt "Объясни квантовую физику" \
     --model "qwen3.5-9b-mlx"
   ```

## Структура проекта

```
ai_advent/
├── Cargo.toml          # Зависимости и метаданные проекта
├── src/
│   ├── lib.rs          # Библиотека бизнес-логики (переиспользуемая)
│   └── main.rs         # CLI обертка
├── .env.example        # Шаблон файла окружения
├── .gitignore
└── README.md
```

## Использование как библиотеки

Библиотеку можно использовать в других проектах:

```rust
use ai_cli_assistant::{ClientConfig, OpenAIClient, LLMRequest};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let client = OpenAIClient::new(ClientConfig {
        api_key: "your-api-key".to_string(),
        base_url: None,
    });

    let request = LLMRequest {
        prompt: "Объясни теорию относительности".to_string(),
        model: "gpt-4o".to_string(),
        system_prompt: Some("Ты опытный преподаватель физики".to_string()),
        max_completion_tokens: Some(500),
        ..Default::default()
    };

    let response = client.chat(request).await?;
    println!("{}", response.content);

    Ok(())
}
```

## Кроссплатформенность

| Платформа | Статус | Примечания |
|-----------|--------|------------|
| macOS     | ✅     | Нативная компиляция |
| Linux     | ✅     | Нативная компиляция |
| Windows   | ✅     | Нативная компиляция |
| WASM      | 🔄     | Требуется адаптация HTTP бекенда |
| iOS/Android | 🔄   | Как статическая библиотека |

## Лицензия

MIT
