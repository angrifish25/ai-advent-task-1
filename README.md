# AI CLI Assistant

Кроссплатформенный CLI клиент для работы с языковыми моделями OpenAI и совместимыми сервисами на Rust.

## Особенности

- **Субкоманды**: `ask`, `stream`, `chat` — каждая со своим поведением
- **Разделение логики**: Бизнес-логика вынесена в отдельную библиотеку (`lib.rs`)
- **Переиспользуемость**: Библиотеку можно использовать в WASM, мобильных приложениях или бэкенде
- **Поддержка локальных моделей**: LM Studio, Ollama, vLLM через кастомный базовый URL
- **Безопасность**: API ключи загружаются только из окружения — в бинарник не вшиваются
- **Вшитые дефолты**: Параметры модели из `.env` встраиваются в бинарник при сборке
- **Асинхронность**: Неблокирующий ввод-вывод через Tokio
- **Стриминг**: Вывод токенов по мере генерации с возможностью прерывания
- **Режим диалога**: Многоходовой чат с сохранением истории сообщений

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

### Переменные окружения

Параметры делятся на два типа по поведению:

**Только рантайм** (не вшиваются в бинарник — секреты):

| Переменная | Обязательная | Описание |
|------------|--------------|----------|
| `OPENAI_API_KEY` | ✅ | API ключ (для локальных моделей — любое значение) |
| `OPENAI_BASE_URL` | ❌ | Базовый URL API (по умолчанию — OpenAI cloud) |

**Параметры модели** (вшиваются в бинарник при сборке, переопределяются в рантайме):

| Переменная | Описание |
|------------|----------|
| `OPENAI_DEFAULT_MODEL` | Модель по умолчанию |
| `LLM_SYSTEM_PROMPT` | Системная инструкция по умолчанию |
| `LLM_MAX_TOKENS` | Максимальное количество токенов |
| `LLM_TEMPERATURE` | Температура сэмплирования (0.0–2.0) |
| `LLM_TOP_P` | Nucleus sampling (0.0–1.0) |
| `LLM_FREQUENCY_PENALTY` | Штраф за повторяющиеся токены (-2.0–2.0) |
| `LLM_PRESENCE_PENALTY` | Штраф за повторяющиеся темы (-2.0–2.0) |
| `LLM_SEED` | Seed для воспроизводимых ответов |
| `LLM_JSON_SCHEMA` | JSON Schema для структурированных ответов (строка JSON) |

### Приоритет параметров

```
CLI-флаг > ENV (рантайм) > вшитый дефолт из .env на момент сборки > встроенное умолчание
```

Это позволяет собрать бинарник с преднастроенными значениями и при этом гибко переопределять их в рантайме или через флаги.

### Примеры конфигурации

#### OpenAI (cloud)
```bash
OPENAI_API_KEY=sk-...
OPENAI_DEFAULT_MODEL=gpt-4o
LLM_TEMPERATURE=0.7
```

#### LM Studio (локально)
```bash
OPENAI_API_KEY=not-needed
OPENAI_BASE_URL=http://localhost:1234/v1
OPENAI_DEFAULT_MODEL=qwen3.5-9b-mlx
LLM_TEMPERATURE=0.3
```

#### Ollama (локально)
```bash
OPENAI_API_KEY=ollama
OPENAI_BASE_URL=http://localhost:11434/v1
```

## Использование

### Справка
```bash
./target/release/ai-cli-assistant --help
./target/release/ai-cli-assistant ask --help
./target/release/ai-cli-assistant stream --help
./target/release/ai-cli-assistant chat --help
```

### Субкоманды

| Субкоманда | Описание |
|------------|----------|
| `ask` | Одиночный запрос — получить ответ и выйти |
| `stream` | Стриминг токенов по мере генерации |
| `chat` | Интерактивный диалог с сохранением истории |

### Общие параметры (доступны во всех субкомандах)

| Флаг | Короткий | ENV-переменная | Описание |
|------|----------|----------------|----------|
| `--prompt` | `-p` | — | Текст запроса (**обязательный**) |
| `--model` | `-m` | `OPENAI_DEFAULT_MODEL` | Название модели |
| `--system` | `-s` | `LLM_SYSTEM_PROMPT` | Системная инструкция |
| `--max-tokens` | | `LLM_MAX_TOKENS` | Ограничение длины ответа |
| `--response-format` | | — | Формат: `text` \| `json-schema` |
| `--json-schema` | | `LLM_JSON_SCHEMA` | JSON Schema объект (строка JSON). Используется с `--response-format json-schema` |
| `--stop` | | — | Stop-последовательность (до 4 раз) |
| `--temperature` | | `LLM_TEMPERATURE` | Случайность ответа (0.0–2.0) |
| `--top-p` | | `LLM_TOP_P` | Nucleus sampling (0.0–1.0) |
| `--frequency-penalty` | | `LLM_FREQUENCY_PENALTY` | Штраф за повторяющиеся токены (-2.0–2.0) |
| `--presence-penalty` | | `LLM_PRESENCE_PENALTY` | Штраф за повторяющиеся темы (-2.0–2.0) |
| `--seed` | | `LLM_SEED` | Seed для воспроизводимых ответов |

### Параметры субкоманды `ask`

| Флаг | Описание |
|------|----------|
| `--show-usage` | Показать статистику использования токенов |

### Глобальные параметры

| Флаг | Короткий | Описание |
|------|----------|----------|
| `--verbose` | `-v` | Режим отладки: показывает конфигурацию и параметры запроса |

---

### ask — одиночный запрос

```bash
./target/release/ai-cli-assistant ask --prompt "Привет, как дела?"
```

С выбором модели и параметрами генерации:
```bash
./target/release/ai-cli-assistant ask \
  --prompt "Напиши функцию сортировки на Rust" \
  --model "gpt-4o" \
  --temperature 0.3 \
  --max-tokens 500
```

Воспроизводимый ответ:
```bash
./target/release/ai-cli-assistant ask \
  --prompt "Придумай имя для переменной" \
  --seed 42 \
  --temperature 0.0
```

Получить ответ в формате JSON (без схемы — модель сама выбирает структуру):
```bash
./target/release/ai-cli-assistant ask \
  --prompt "Верни список из 5 языков программирования в виде JSON-массива" \
  --response-format json-schema
```

Получить ответ в формате JSON с явной схемой:
```bash
./target/release/ai-cli-assistant ask \
  --prompt "Верни список из 5 языков программирования" \
  --response-format json-schema \
  --json-schema '{"type":"object","properties":{"languages":{"type":"array","items":{"type":"string"}}},"required":["languages"],"additionalProperties":false}'
```

Через `.env` (схема применяется ко всем запросам с `--response-format json-schema`):
```bash
LLM_JSON_SCHEMA={"type":"object","properties":{"answer":{"type":"string"}},"required":["answer"]}
```

Stop-последовательности:
```bash
./target/release/ai-cli-assistant ask \
  --prompt "Напиши инструкцию по пунктам" \
  --stop "###" --stop "END"
```

Показать использование токенов:
```bash
./target/release/ai-cli-assistant ask \
  --prompt "Объясни квантовую физику" \
  --show-usage
```

### stream — стриминг ответа

```bash
./target/release/ai-cli-assistant stream \
  --prompt "Напиши стихотворение про Rust"
```

Нажмите любую клавишу во время генерации — стриминг остановится.

### chat — интерактивный диалог

```bash
./target/release/ai-cli-assistant chat \
  --prompt "Привет! Давай поговорим о Rust."
```

После каждого ответа модели появится приглашение `Вы:`. Пустая строка завершает диалог. История сообщений сохраняется на протяжении всего сеанса.

С системным промптом и параметрами:
```bash
./target/release/ai-cli-assistant chat \
  --prompt "Помоги разобраться с async/await" \
  --system "Ты опытный Rust-разработчик. Отвечай кратко и с примерами кода." \
  --model "gpt-4o" \
  --temperature 0.2
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
   ./target/release/ai-cli-assistant chat \
     --prompt "Объясни принципы SOLID" \
     --model "qwen3.5-9b-mlx"
   ```

## Структура проекта

```
ai_advent/
├── build.rs            # Встраивание параметров из .env в бинарник при сборке
├── Cargo.toml          # Зависимости и метаданные проекта
├── src/
│   ├── lib.rs          # Библиотека бизнес-логики (переиспользуемая)
│   └── main.rs         # CLI: субкоманды ask / stream / chat
├── .env                # Конфигурация (не коммитится)
├── .env.example        # Шаблон конфигурации
├── .gitignore
└── README.md
```

## Использование как библиотеки

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
        temperature: Some(0.7),
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
