//! Product database protobuf serialization.
//!
//! The Blizzard Agent persists product installation state as protobuf
//! messages in SQLite-backed files (`product.db` and `.product.db`).
//! This module re-exports the prost-generated `proto_database` types
//! and provides SQLite persistence functions.
//!
//! Two database files exist:
//! - `product.db` -- main agent database (SQLite with a `Database` blob)
//! - `.product.db` -- per-install file (raw `ProductInstall` message)

use prost::Message;

use crate::error::{ContainerlessError, ContainerlessResult};

// Re-export all generated types so existing consumers keep working.
pub use cascette_proto::proto_database::*;

// Also re-export the top-level Database with its original name for
// the lib.rs `pub use product_db::Database as ProductDatabase` alias.
pub use cascette_proto::proto_database::Database;

// ─── SQLite Storage ─────────────────────────────────────────────────

/// Read a product database from a SQLite connection.
///
/// Reads the protobuf bytes from `SELECT data FROM product WHERE id = 1`
/// and deserializes into a `Database` message.
pub async fn product_db_read(conn: &turso::Connection) -> ContainerlessResult<Option<Database>> {
    let mut rows = conn
        .query("SELECT data FROM product WHERE id = 1", ())
        .await
        .map_err(ContainerlessError::Database)?;

    let Some(row) = rows.next().await.map_err(ContainerlessError::Database)? else {
        return Ok(None);
    };

    let data: Vec<u8> = row
        .get::<Vec<u8>>(0)
        .map_err(ContainerlessError::Database)?;

    let db = Database::decode(data.as_slice())
        .map_err(|e| ContainerlessError::Integrity(format!("protobuf decode: {e}")))?;
    Ok(Some(db))
}

/// Write a product database to a SQLite connection.
///
/// Serializes the `Database` message to protobuf bytes and stores via
/// `INSERT OR REPLACE INTO product (id, data) VALUES (1, ?)`.
pub async fn product_db_write(conn: &turso::Connection, db: &Database) -> ContainerlessResult<()> {
    let data = db.encode_to_vec();

    // Ensure the table exists.
    conn.execute(
        "CREATE TABLE IF NOT EXISTS product (id INTEGER PRIMARY KEY, data BLOB)",
        (),
    )
    .await
    .map_err(ContainerlessError::Database)?;

    conn.execute(
        "INSERT OR REPLACE INTO product (id, data) VALUES (1, ?1)",
        turso::params![data],
    )
    .await
    .map_err(ContainerlessError::Database)?;

    Ok(())
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used)]
mod tests {
    use super::*;

    #[test]
    fn test_language_setting_roundtrip() {
        let setting = LanguageSetting {
            language: Some("enUS".to_string()),
            option: Some(LanguageOption::LangoptionText.into()),
        };
        let data = setting.encode_to_vec();
        let decoded = LanguageSetting::decode(data.as_slice()).unwrap();
        assert_eq!(decoded.language.as_deref(), Some("enUS"));
        assert_eq!(decoded.option, Some(LanguageOption::LangoptionText.into()));
    }

    #[test]
    fn test_user_settings_roundtrip() {
        let settings = UserSettings {
            install_path: Some("/opt/games/wow".to_string()),
            play_region: Some("us".to_string()),
            desktop_shortcut: Some(ShortcutOption::ShortcutUser.into()),
            startmenu_shortcut: Some(ShortcutOption::ShortcutUser.into()),
            languages: vec![
                LanguageSetting {
                    language: Some("enUS".to_string()),
                    option: Some(LanguageOption::LangoptionText.into()),
                },
                LanguageSetting {
                    language: Some("deDE".to_string()),
                    option: Some(LanguageOption::LangoptionSpeech.into()),
                },
            ],
            selected_text_language: Some("enUS".to_string()),
            selected_speech_language: Some("enUS".to_string()),
            ..Default::default()
        };
        let data = settings.encode_to_vec();
        let decoded = UserSettings::decode(data.as_slice()).unwrap();
        assert_eq!(decoded.install_path.as_deref(), Some("/opt/games/wow"));
        assert_eq!(decoded.languages.len(), 2);
        assert_eq!(decoded.languages[1].language.as_deref(), Some("deDE"));
        assert_eq!(
            decoded.languages[1].option,
            Some(LanguageOption::LangoptionSpeech.into())
        );
    }

    #[test]
    fn test_build_config_roundtrip() {
        let config = BuildConfig {
            region: Some("us".to_string()),
            build_config: Some("abc123".to_string()),
        };
        let data = config.encode_to_vec();
        let decoded = BuildConfig::decode(data.as_slice()).unwrap();
        assert_eq!(decoded.region.as_deref(), Some("us"));
        assert_eq!(decoded.build_config.as_deref(), Some("abc123"));
    }

    #[test]
    fn test_base_product_state_roundtrip() {
        let state = BaseProductState {
            installed: Some(true),
            playable: Some(true),
            update_complete: Some(false),
            background_download_available: Some(true),
            current_version_str: Some("1.13.2.31650".to_string()),
            installed_build_config: vec![BuildConfig {
                region: Some("us".to_string()),
                build_config: Some("build-key-123".to_string()),
            }],
            decryption_key: Some(String::new()),
            completed_install_actions: vec!["action1".to_string(), "action2".to_string()],
            ..Default::default()
        };
        let data = state.encode_to_vec();
        let decoded = BaseProductState::decode(data.as_slice()).unwrap();
        assert_eq!(decoded.installed, Some(true));
        assert_eq!(decoded.playable, Some(true));
        assert_eq!(decoded.update_complete, Some(false));
        assert_eq!(decoded.current_version_str.as_deref(), Some("1.13.2.31650"));
        assert_eq!(
            decoded.installed_build_config[0].build_config.as_deref(),
            Some("build-key-123")
        );
        assert_eq!(decoded.completed_install_actions.len(), 2);
    }

    #[test]
    fn test_product_operations_roundtrip() {
        let ops = ProductOperations {
            active_operation: Some(Operation::OpUpdate.into()),
            priority: Some(5),
        };
        let data = ops.encode_to_vec();
        let decoded = ProductOperations::decode(data.as_slice()).unwrap();
        assert_eq!(decoded.active_operation, Some(Operation::OpUpdate.into()));
        assert_eq!(decoded.priority, Some(5));
    }

    #[test]
    fn test_product_install_roundtrip() {
        let install = ProductInstall {
            uid: Some("wow-uid-123".to_string()),
            product_code: Some("wow".to_string()),
            settings: Some(UserSettings {
                install_path: Some("/opt/wow".to_string()),
                play_region: Some("us".to_string()),
                ..Default::default()
            }),
            cached_product_state: Some(CachedProductState {
                base_product_state: Some(BaseProductState {
                    installed: Some(true),
                    playable: Some(true),
                    ..Default::default()
                }),
                ..Default::default()
            }),
            ..Default::default()
        };
        let data = install.encode_to_vec();
        let decoded = ProductInstall::decode(data.as_slice()).unwrap();
        assert_eq!(decoded.uid.as_deref(), Some("wow-uid-123"));
        assert_eq!(decoded.product_code.as_deref(), Some("wow"));
        assert_eq!(
            decoded
                .cached_product_state
                .unwrap()
                .base_product_state
                .unwrap()
                .installed,
            Some(true)
        );
    }

    #[test]
    fn test_database_roundtrip() {
        let db = Database {
            product_install: vec![
                ProductInstall {
                    uid: Some("wow-1".to_string()),
                    product_code: Some("wow".to_string()),
                    settings: Some(UserSettings {
                        install_path: Some("/opt/wow".to_string()),
                        ..Default::default()
                    }),
                    ..Default::default()
                },
                ProductInstall {
                    uid: Some("d2r-1".to_string()),
                    product_code: Some("d2r".to_string()),
                    ..Default::default()
                },
            ],
            active_processes: vec![ActiveProcess {
                process_name: Some("wow.exe".to_string()),
                pid: Some(12345),
                ..Default::default()
            }],
            product_configs: vec![ProductConfig {
                product_code: Some("wow".to_string()),
                metadata_hash: Some("abc123".to_string()),
            }],
            download_settings: Some(DownloadSettings {
                download_limit: Some(10_000_000),
                backfill_limit: Some(5_000_000),
                backfill_limit_uses_default: Some(true),
            }),
            shared_components: vec![SharedComponent {
                base: ProductInstall {
                    uid: Some("shared".to_string()),
                    product_code: Some("shared".to_string()),
                    settings: Some(UserSettings {
                        install_path: Some("/opt/shared".to_string()),
                        ..Default::default()
                    }),
                    ..Default::default()
                },
                dependent_uid: vec!["wow-1".to_string()],
            }],
            ..Default::default()
        };

        let data = db.encode_to_vec();
        let decoded = Database::decode(data.as_slice()).unwrap();

        assert_eq!(decoded.product_install.len(), 2);
        assert_eq!(decoded.product_install[0].uid.as_deref(), Some("wow-1"));
        assert_eq!(
            decoded.product_install[1].product_code.as_deref(),
            Some("d2r")
        );
        assert_eq!(decoded.active_processes.len(), 1);
        assert_eq!(decoded.active_processes[0].pid, Some(12345));
        assert_eq!(
            decoded.product_configs[0].product_code.as_deref(),
            Some("wow")
        );
        assert_eq!(
            decoded.download_settings.as_ref().unwrap().download_limit,
            Some(10_000_000)
        );
        assert_eq!(
            decoded
                .download_settings
                .as_ref()
                .unwrap()
                .backfill_limit_uses_default,
            Some(true)
        );
        assert_eq!(decoded.shared_components.len(), 1);
        assert_eq!(decoded.shared_components[0].dependent_uid, vec!["wow-1"]);
    }

    #[test]
    fn test_empty_database_roundtrip() {
        let db = Database::default();
        let data = db.encode_to_vec();
        assert!(data.is_empty());

        let decoded = Database::decode(data.as_slice()).unwrap();
        assert!(decoded.product_install.is_empty());
    }

    #[test]
    fn test_cached_product_state_with_all_progress() {
        let state = CachedProductState {
            base_product_state: Some(BaseProductState {
                installed: Some(true),
                ..Default::default()
            }),
            backfill_progress: Some(BackfillProgress {
                progress: Some(0.5),
                backgrounddownload: Some(true),
                paused: Some(false),
                details: Some(BuildProgressDetails {
                    target_key: Some("key1".to_string()),
                    ..Default::default()
                }),
                ..Default::default()
            }),
            repair_progress: Some(RepairProgress {
                progress: Some(0.25),
            }),
            update_progress: Some(UpdateProgress {
                progress: Some(0.9),
                last_disc_set_used: Some("disc3".to_string()),
                disc_ignored: Some(false),
                ..Default::default()
            }),
        };

        let data = state.encode_to_vec();
        let decoded = CachedProductState::decode(data.as_slice()).unwrap();

        assert_eq!(decoded.base_product_state.unwrap().installed, Some(true));

        let backfill = decoded.backfill_progress.unwrap();
        assert_eq!(backfill.progress, Some(0.5));
        assert_eq!(backfill.backgrounddownload, Some(true));
        assert_eq!(
            backfill.details.unwrap().target_key.as_deref(),
            Some("key1")
        );

        let repair = decoded.repair_progress.unwrap();
        assert_eq!(repair.progress, Some(0.25));

        let update = decoded.update_progress.unwrap();
        assert_eq!(update.last_disc_set_used.as_deref(), Some("disc3"));
    }

    #[test]
    fn test_download_settings_roundtrip() {
        let settings = DownloadSettings {
            download_limit: Some(0),
            backfill_limit: Some(1_000_000),
            backfill_limit_uses_default: Some(true),
        };
        let data = settings.encode_to_vec();
        let decoded = DownloadSettings::decode(data.as_slice()).unwrap();
        assert_eq!(decoded.download_limit, Some(0));
        assert_eq!(decoded.backfill_limit, Some(1_000_000));
        assert_eq!(decoded.backfill_limit_uses_default, Some(true));
    }

    #[test]
    fn test_shared_component_roundtrip() {
        let comp = SharedComponent {
            base: ProductInstall {
                uid: Some("shared_lib".to_string()),
                product_code: Some("shared_lib".to_string()),
                settings: Some(UserSettings {
                    install_path: Some("/opt/shared/lib".to_string()),
                    ..Default::default()
                }),
                ..Default::default()
            },
            dependent_uid: vec!["dep1".to_string(), "dep2".to_string()],
        };
        let data = comp.encode_to_vec();
        let decoded = SharedComponent::decode(data.as_slice()).unwrap();
        assert_eq!(decoded.base.uid.as_deref(), Some("shared_lib"));
        assert_eq!(decoded.dependent_uid.len(), 2);
    }

    #[test]
    fn test_install_handshake_roundtrip() {
        let handshake = InstallHandshake {
            uid: Some("install-uid".to_string()),
            product: Some("wow".to_string()),
            settings: Some(UserSettings {
                install_path: Some("/opt/wow".to_string()),
                ..Default::default()
            }),
            priority: Some(1),
        };
        let data = handshake.encode_to_vec();
        let decoded = InstallHandshake::decode(data.as_slice()).unwrap();
        assert_eq!(decoded.uid.as_deref(), Some("install-uid"));
        assert_eq!(decoded.product.as_deref(), Some("wow"));
        assert_eq!(
            decoded.settings.unwrap().install_path.as_deref(),
            Some("/opt/wow")
        );
    }

    #[tokio::test]
    async fn test_sqlite_roundtrip() {
        let turso_db = turso::Builder::new_local(":memory:").build().await.unwrap();
        let conn = turso_db.connect().unwrap();

        let db = Database {
            product_install: vec![ProductInstall {
                uid: Some("test-uid".to_string()),
                product_code: Some("wow".to_string()),
                settings: Some(UserSettings {
                    install_path: Some("/opt/wow".to_string()),
                    ..Default::default()
                }),
                cached_product_state: Some(CachedProductState {
                    base_product_state: Some(BaseProductState {
                        installed: Some(true),
                        playable: Some(true),
                        current_version_str: Some("1.0.0".to_string()),
                        ..Default::default()
                    }),
                    ..Default::default()
                }),
                ..Default::default()
            }],
            ..Default::default()
        };

        product_db_write(&conn, &db).await.unwrap();

        let loaded = product_db_read(&conn).await.unwrap().unwrap();
        assert_eq!(loaded.product_install.len(), 1);
        assert_eq!(loaded.product_install[0].uid.as_deref(), Some("test-uid"));
        assert_eq!(
            loaded.product_install[0].product_code.as_deref(),
            Some("wow")
        );
        assert_eq!(
            loaded.product_install[0]
                .cached_product_state
                .as_ref()
                .unwrap()
                .base_product_state
                .as_ref()
                .unwrap()
                .installed,
            Some(true)
        );
    }

    #[tokio::test]
    async fn test_sqlite_read_empty() {
        let turso_db = turso::Builder::new_local(":memory:").build().await.unwrap();
        let conn = turso_db.connect().unwrap();

        // Create the table but don't insert anything.
        conn.execute(
            "CREATE TABLE IF NOT EXISTS product (id INTEGER PRIMARY KEY, data BLOB)",
            (),
        )
        .await
        .unwrap();

        let result = product_db_read(&conn).await.unwrap();
        assert!(result.is_none());
    }
}
