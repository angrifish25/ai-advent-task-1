# AI CLI Assistant

Кроссплатформенный CLI клиент для работы с языковыми моделями OpenAI и совместимыми сервисами на Rust.

## Особенности

- **Разделение логики**: Бизнес-логика вынесена в отдельную библиотеку (`lib.rs`)
- **Переиспользуемость**: Библиотеку можно использовать в WASM, мобильных приложениях или бэкенде
- **Поддержка локальных моделей**: LM Studio, Ollama, vLLM через кастомный базовый URL
- **Безопасность**: API ключи загружаются из переменных окружения
- **Асинхронность**: Неблокирующий ввод-вывод через Tokio
- **Типобезопасность**: Строгая типизация запросов и ответов

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

### Примеры конфигурации

#### OpenAI (cloud)
```bash
OPENAI_API_KEY=sk-...
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

### Базовый запрос
```bash
# macOS / Linux
./target/release/ai-cli-assistant --prompt "Привет, как дела?"

# Windows
.\target\release\ai-cli-assistant.exe --prompt "Привет, как дела?"
```

### С выбором модели
```bash
./target/release/ai-cli-assistant \
  --prompt "Напиши функцию на Python" \
  --model "gpt-4"
```

### С локальной моделью (LM Studio)
1. Запустите LM Studio и загрузите модель (например, `qwen3.5-9b-mlx`)
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

### С системным промптом
```bash
./target/release/ai-cli-assistant \
  --prompt "Переведи: Hello, world!" \
  --system "Ты профессиональный переводчик. Переводи только текст, без объяснений."
```

### Режим отладки
```bash
./target/release/ai-cli-assistant \
  --prompt "Тест" \
  --verbose
```

### Показать использование токенов
```bash
./target/release/ai-cli-assistant \
  --prompt "Объясни квантовую физику" \
  --show-usage
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
use ai_cli_assistant::{OpenAIClient, LLMRequest};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let client = OpenAIClient::new("your-api-key");
    
    let request = LLMRequest {
        prompt: "Объясни теорию относительности".to_string(),
        model: "gpt-3.5-turbo".to_string(),
        system_prompt: Some("Ты опытный преподаватель физики".to_string()),
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
