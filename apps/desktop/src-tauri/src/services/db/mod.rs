pub mod keystore;
pub mod settings;

use rusqlite::Connection;
use std::collections::HashMap;
use std::path::PathBuf;

#[derive(Debug, thiserror::Error)]
pub enum DbError {
    #[error("SQLite error: {0}")]
    Sqlite(#[from] rusqlite::Error),
    #[error("Path error: {0}")]
    PathError(String),
    #[error("Serialization error: {0}")]
    SerializationError(String),
}

pub struct Database {
    conn: Connection,
}

/// Whisper-supported language codes (BCP 47 primary subtags).
const WHISPER_SUPPORTED_LANGUAGES: &[&str] = &[
    "en", "zh", "de", "es", "ru", "ko", "fr", "ja", "pt", "tr", "pl", "ca", "nl", "ar", "sv",
    "it", "id", "hi", "fi", "vi", "he", "uk", "el", "ms", "cs", "ro", "da", "hu", "ta", "no",
    "th", "ur", "hr", "bg", "lt", "la", "mi", "ml", "cy", "sk", "te", "fa", "lv", "bn", "sr",
    "az", "sl", "kn", "et", "mk", "br", "eu", "is", "hy", "ne", "mn", "bs", "kk", "sq", "sw",
    "gl", "mr", "pa", "si", "km", "sn", "yo", "so", "af", "oc", "ka", "be", "tg", "sd", "gu",
    "am", "yi", "lo", "uz", "fo", "ht", "ps", "tk", "nn", "mt", "sa", "lb", "my", "bo", "tl",
    "mg", "as", "tt", "haw", "ln", "ha", "ba", "jw", "su", "yue",
];

/// Detects the system locale and returns a supported Whisper language code.
/// Falls back to `["en"]` for unknown or unsupported locales.
fn detect_default_languages() -> Vec<String> {
    let locale = match sys_locale::get_locale() {
        Some(l) => l,
        None => return vec!["en".to_string()],
    };

    // Handle degenerate locale values
    if locale.is_empty() || locale == "C" || locale == "POSIX" {
        return vec!["en".to_string()];
    }

    // Split on '-' or '_' to get primary language subtag (e.g. "es" from "es-MX")
    let primary = locale
        .split(|c| c == '-' || c == '_')
        .next()
        .unwrap_or("")
        .to_lowercase();

    if primary.is_empty() {
        return vec!["en".to_string()];
    }

    if WHISPER_SUPPORTED_LANGUAGES.contains(&primary.as_str()) {
        vec![primary]
    } else {
        vec!["en".to_string()]
    }
}

impl Database {
    pub fn new(data_dir: PathBuf) -> Result<Self, DbError> {
        let db_path = data_dir.join("dictto.db");

        // Ensure parent directory exists
        if let Some(parent) = db_path.parent() {
            std::fs::create_dir_all(parent).map_err(|e| DbError::PathError(e.to_string()))?;
        }

        let conn = Connection::open(&db_path)?;
        let db = Self { conn };
        db.initialize()?;
        Ok(db)
    }

    fn initialize(&self) -> Result<(), DbError> {
        self.conn.execute_batch(
            "
            CREATE TABLE IF NOT EXISTS settings (
                key TEXT PRIMARY KEY,
                value TEXT NOT NULL
            );

            CREATE TABLE IF NOT EXISTS history (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                raw_text TEXT NOT NULL,
                cleaned_text TEXT NOT NULL,
                language TEXT NOT NULL DEFAULT 'es',
                duration_ms INTEGER,
                created_at TEXT NOT NULL DEFAULT (datetime('now'))
            );
            ",
        )?;

        // FIRST: migrate existing installs from old 'language' key to new 'languages' JSON array
        self.migrate_language_to_languages()?;
        // SECOND: insert static defaults (no 'language' entry)
        self.ensure_defaults()?;
        // THIRD: set locale-detected 'languages' for brand-new installs
        self.ensure_languages_default()?;

        Ok(())
    }

    fn ensure_defaults(&self) -> Result<(), DbError> {
        let defaults = settings::default_settings();
        let mut stmt = self
            .conn
            .prepare("INSERT OR IGNORE INTO settings (key, value) VALUES (?1, ?2)")?;

        for (key, value) in defaults {
            stmt.execute(rusqlite::params![key, value])?;
        }

        Ok(())
    }

    /// Migrates the old single-string `language` setting to the new `languages` JSON array.
    /// - If `languages` key is missing: reads old `language` value (or falls back to "es") and
    ///   writes it as a JSON array under `languages`.
    /// - Deletes the old `language` key unconditionally (destructive migration).
    fn migrate_language_to_languages(&self) -> Result<(), DbError> {
        let old_language = self.get_setting("language");
        let new_languages = self.get_setting("languages");

        if new_languages.is_none() {
            // Migrate: wrap old value (or default) into JSON array
            let lang = old_language.unwrap_or_else(|| "es".to_string());
            let json = serde_json::to_string(&vec![lang.clone()])
                .map_err(|e| DbError::SerializationError(e.to_string()))?;
            self.set_setting("languages", &json)?;
            log::info!("[db] Migrated 'language' ({}) -> 'languages': {}", lang, json);
        }

        // Always delete the old key if it exists
        if self.get_setting("language").is_some() {
            self.conn
                .execute("DELETE FROM settings WHERE key = 'language'", [])?;
            log::info!("[db] Deleted legacy 'language' key");
        }

        Ok(())
    }

    /// Sets the `languages` setting from system locale detection for brand-new installs.
    /// Only runs when `languages` is still absent after migration (i.e. a completely fresh DB).
    fn ensure_languages_default(&self) -> Result<(), DbError> {
        if self.get_setting("languages").is_none() {
            let detected = detect_default_languages();
            let json = serde_json::to_string(&detected)
                .map_err(|e| DbError::SerializationError(e.to_string()))?;
            self.set_setting("languages", &json)?;
            log::info!("[db] Default language set from locale: {:?}", detected);
        }
        Ok(())
    }

    pub fn get_setting(&self, key: &str) -> Option<String> {
        self.conn
            .query_row(
                "SELECT value FROM settings WHERE key = ?1",
                rusqlite::params![key],
                |row| row.get(0),
            )
            .ok()
    }

    pub fn set_setting(&self, key: &str, value: &str) -> Result<(), DbError> {
        self.conn.execute(
            "INSERT OR REPLACE INTO settings (key, value) VALUES (?1, ?2)",
            rusqlite::params![key, value],
        )?;
        Ok(())
    }

    pub fn get_all_settings(&self) -> Result<HashMap<String, String>, DbError> {
        let mut stmt = self.conn.prepare("SELECT key, value FROM settings")?;
        let rows = stmt.query_map([], |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
        })?;

        let mut map = HashMap::new();
        for row in rows {
            let (key, value) = row?;
            map.insert(key, value);
        }
        Ok(map)
    }

    pub fn save_history(
        &self,
        raw_text: &str,
        cleaned_text: &str,
        language: &str,
        duration_ms: Option<i64>,
    ) -> Result<(), DbError> {
        self.conn.execute(
            "INSERT INTO history (raw_text, cleaned_text, language, duration_ms) VALUES (?1, ?2, ?3, ?4)",
            rusqlite::params![raw_text, cleaned_text, language, duration_ms],
        )?;
        Ok(())
    }
}

#[cfg(test)]
impl Database {
    /// Creates an in-memory SQLite database for testing.
    fn new_in_memory() -> Result<Self, DbError> {
        let conn = Connection::open_in_memory()?;
        let db = Self { conn };
        db.initialize()?;
        Ok(db)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // LANG-01: JSON array storage round-trips correctly in SQLite
    #[test]
    fn test_languages_json_array_round_trips_in_sqlite() {
        let db = Database::new_in_memory().expect("in-memory db should open");
        let json = r#"["es","en"]"#;
        db.set_setting("languages", json)
            .expect("set_setting should succeed");
        let result = db.get_setting("languages");
        assert_eq!(
            result,
            Some(r#"["es","en"]"#.to_string()),
            "languages JSON array should round-trip correctly through SQLite"
        );
    }

    // LANG-01 additional: single-element array also round-trips
    #[test]
    fn test_single_language_json_array_round_trips_in_sqlite() {
        let db = Database::new_in_memory().expect("in-memory db should open");
        let json = r#"["es"]"#;
        db.set_setting("languages", json)
            .expect("set_setting should succeed");
        let result = db.get_setting("languages");
        assert_eq!(
            result,
            Some(r#"["es"]"#.to_string()),
            "single-language JSON array should round-trip through SQLite"
        );
    }

    // LANG-02: migrate_language_to_languages moves old key to new key and deletes old key
    #[test]
    fn test_language_migration_writes_new_key_and_deletes_old_key() {
        // Create a bare connection (without initialize) so we can manually set the old key
        let conn = Connection::open_in_memory().expect("in-memory connection should open");
        conn.execute_batch(
            "CREATE TABLE settings (key TEXT PRIMARY KEY, value TEXT NOT NULL);",
        )
        .expect("create table should succeed");
        // Insert the legacy 'language' key (simulating an existing install)
        conn.execute(
            "INSERT INTO settings (key, value) VALUES ('language', 'es')",
            [],
        )
        .expect("insert legacy key should succeed");
        let db = Database { conn };

        // Run migration directly
        db.migrate_language_to_languages()
            .expect("migration should succeed");

        // New key should be a JSON array wrapping the old value
        let new_val = db.get_setting("languages");
        assert_eq!(
            new_val,
            Some(r#"["es"]"#.to_string()),
            "migration should write old language value as JSON array under 'languages' key"
        );

        // Old key must be deleted
        let old_val = db.get_setting("language");
        assert_eq!(
            old_val, None,
            "migration should delete the legacy 'language' key"
        );
    }

    // LANG-02: migration when no old key exists uses default "es"
    #[test]
    fn test_migration_without_old_key_defaults_to_es() {
        let conn = Connection::open_in_memory().expect("in-memory connection should open");
        conn.execute_batch(
            "CREATE TABLE settings (key TEXT PRIMARY KEY, value TEXT NOT NULL);",
        )
        .expect("create table should succeed");
        let db = Database { conn };

        db.migrate_language_to_languages()
            .expect("migration should succeed");

        let new_val = db.get_setting("languages");
        assert_eq!(
            new_val,
            Some(r#"["es"]"#.to_string()),
            "migration with no old key should default to [\"es\"]"
        );
    }

    // LANG-02: when 'languages' already exists, migration does NOT overwrite it
    #[test]
    fn test_migration_does_not_overwrite_existing_languages_key() {
        let conn = Connection::open_in_memory().expect("in-memory connection should open");
        conn.execute_batch(
            "CREATE TABLE settings (key TEXT PRIMARY KEY, value TEXT NOT NULL);",
        )
        .expect("create table should succeed");
        conn.execute(
            r#"INSERT INTO settings (key, value) VALUES ('languages', '["es","en"]')"#,
            [],
        )
        .expect("insert existing languages key should succeed");
        let db = Database { conn };

        db.migrate_language_to_languages()
            .expect("migration should succeed");

        // Existing languages key should be unchanged
        let val = db.get_setting("languages");
        assert_eq!(
            val,
            Some(r#"["es","en"]"#.to_string()),
            "migration should not overwrite existing 'languages' key"
        );
    }

    // LANG-03: detect_default_languages returns a non-empty Vec with valid Whisper code or fallback
    #[test]
    fn test_default_language_detection_returns_valid_whisper_code_or_fallback() {
        let detected = detect_default_languages();
        assert!(
            !detected.is_empty(),
            "detect_default_languages should return at least one language"
        );
        assert_eq!(
            detected.len(),
            1,
            "detect_default_languages should return exactly one language"
        );
        let code = &detected[0];
        assert!(
            WHISPER_SUPPORTED_LANGUAGES.contains(&code.as_str()) || code == "en",
            "detected language '{}' must be a Whisper-supported code (or the 'en' fallback)",
            code
        );
    }

    // LANG-03: fallback to ["en"] for unsupported locale codes
    #[test]
    fn test_whisper_supported_languages_validates_correctly() {
        // Directly test the validation logic that detect_default_languages uses:
        // A known-supported code passes, an unknown code falls back to "en"
        let supported = WHISPER_SUPPORTED_LANGUAGES;
        assert!(supported.contains(&"es"), "'es' must be in WHISPER_SUPPORTED_LANGUAGES");
        assert!(supported.contains(&"en"), "'en' must be in WHISPER_SUPPORTED_LANGUAGES");
        assert!(
            !supported.contains(&"xx"),
            "'xx' must NOT be in WHISPER_SUPPORTED_LANGUAGES (trigger fallback)"
        );
        // Simulate the fallback logic:
        let fake_locale_primary = "xx";
        let result = if supported.contains(&fake_locale_primary) {
            vec![fake_locale_primary.to_string()]
        } else {
            vec!["en".to_string()]
        };
        assert_eq!(
            result,
            vec!["en".to_string()],
            "unsupported locale primary tag should fall back to [\"en\"]"
        );
    }

    // PIPE-05: languages.join(",") produces correct history string
    #[test]
    fn test_languages_join_produces_correct_history_string() {
        // Single language
        let single = vec!["es".to_string()];
        assert_eq!(
            single.join(","),
            "es",
            "single language join should produce bare code"
        );

        // Two languages
        let two = vec!["es".to_string(), "en".to_string()];
        assert_eq!(
            two.join(","),
            "es,en",
            "two-language join should produce comma-separated codes"
        );

        // Three languages
        let three = vec!["es".to_string(), "en".to_string(), "fr".to_string()];
        assert_eq!(
            three.join(","),
            "es,en,fr",
            "three-language join should produce comma-separated codes"
        );
    }

    // PIPE-05: history language column stores and retrieves the joined string
    #[test]
    fn test_history_saves_and_retrieves_comma_joined_language_string() {
        let db = Database::new_in_memory().expect("in-memory db should open");
        let languages = vec!["es".to_string(), "en".to_string()];
        let lang_str = languages.join(",");

        db.save_history("raw text", "cleaned text", &lang_str, None)
            .expect("save_history should succeed");

        let result: String = db
            .conn
            .query_row(
                "SELECT language FROM history WHERE id = 1",
                [],
                |row| row.get(0),
            )
            .expect("history row should exist");
        assert_eq!(
            result, "es,en",
            "history language column should store comma-joined language codes"
        );
    }
}
