use std::fs;
use std::path::Path;

fn main() {
    // Перечитываем при любом изменении .env
    println!("cargo:rerun-if-changed=.env");

    let env_file = Path::new(".env");
    if !env_file.exists() {
        return;
    }

    let content = fs::read_to_string(env_file).unwrap_or_default();

    // Только безопасные параметры модели — секреты не вшиваем
    const EMBED_KEYS: &[&str] = &[
        "OPENAI_DEFAULT_MODEL",
        "LLM_SYSTEM_PROMPT",
        "LLM_MAX_TOKENS",
        "LLM_TEMPERATURE",
        "LLM_TOP_P",
        "LLM_FREQUENCY_PENALTY",
        "LLM_PRESENCE_PENALTY",
        "LLM_SEED",
        "LLM_JSON_SCHEMA",
    ];

    for line in content.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        if let Some((key, value)) = line.split_once('=') {
            let key = key.trim();
            let value = value.trim();
            if EMBED_KEYS.contains(&key) && !value.is_empty() {
                println!("cargo:rustc-env=BUILD_DEFAULT_{key}={value}");
            }
        }
    }
}
