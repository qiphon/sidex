//! Settings/configuration management for `SideX`.
//!
//! Provides a layered settings store (default < user < workspace), JSONC
//! parsing, a schema registry for extensions with validation, built-in
//! default values matching VS Code conventions, language-specific overrides,
//! settings migration, profiles, and settings sync.

pub mod defaults;
pub mod jsonc;
pub mod migration;
pub mod profiles;
pub mod schema;
pub mod settings;
pub mod sync;

pub use defaults::builtin_defaults;
pub use jsonc::{
    format_jsonc, modify_jsonc, parse_jsonc, parse_jsonc_with_comments, strip_comments,
    CommentKind, JsoncComment, JsoncError,
};
pub use migration::{migrate_settings, MigrationAction, MigrationRule};
pub use profiles::{
    ExportedProfile, Profile, ProfileExtension, ProfileFlags, ProfileId, ProfileManager,
};
pub use schema::{validate_setting, SchemaRegistry, SettingSchema, SettingScope, SettingType};
pub use settings::Settings;
pub use sync::{
    merge, ConflictResolution, SettingsSync, SyncAccount, SyncAuthProvider, SyncConflict,
    SyncData, SyncDataProvider, SyncResource, SyncResult, SyncState,
};
