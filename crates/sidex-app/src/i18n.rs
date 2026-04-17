//! Internationalization — manages locale and translations for the SideX UI.
//!
//! Supports VS Code's 14 display languages. Translation files use a flat
//! JSON `{ "key": "translated text" }` format. Missing keys fall back to
//! the key itself (or English when the translation file is incomplete).

use std::collections::HashMap;

use serde::{Deserialize, Serialize};

/// Metadata about a supported locale.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LocaleInfo {
    /// IETF BCP-47 language tag (e.g. `"en"`, `"zh-cn"`).
    pub code: String,
    /// English display name.
    pub name: String,
    /// Native display name.
    pub native_name: String,
}

/// Internationalization service managing the active locale and translations.
pub struct I18n {
    locale: String,
    translations: HashMap<String, String>,
    fallback: HashMap<String, String>,
}

impl I18n {
    /// Create a new `I18n` service with translations for the given locale.
    ///
    /// English is always loaded as the fallback. If `locale` is `"en"`,
    /// the primary and fallback maps are the same.
    pub fn new(locale: &str) -> Self {
        let fallback = english_translations();
        let translations = if locale == "en" {
            fallback.clone()
        } else {
            translations_for(locale)
        };
        Self {
            locale: locale.to_owned(),
            translations,
            fallback,
        }
    }

    /// Translate a key. Returns the translated string, falling back to the
    /// English translation, then to the raw key.
    pub fn t<'a>(&'a self, key: &'a str) -> &'a str {
        if let Some(v) = self.translations.get(key) {
            return v.as_str();
        }
        if let Some(v) = self.fallback.get(key) {
            return v.as_str();
        }
        key
    }

    /// Translate a key and perform `{placeholder}` interpolation.
    ///
    /// Each pair in `args` replaces `{name}` with the corresponding value.
    pub fn t_with_args(&self, key: &str, args: &[(&str, &str)]) -> String {
        let mut result = self.t(key).to_owned();
        for &(name, value) in args {
            let placeholder = format!("{{{name}}}");
            result = result.replace(&placeholder, value);
        }
        result
    }

    /// Change the active locale, reloading translations.
    pub fn set_locale(&mut self, locale: &str) {
        self.locale = locale.to_owned();
        self.translations = if locale == "en" {
            self.fallback.clone()
        } else {
            translations_for(locale)
        };
    }

    /// The currently active locale code.
    pub fn locale(&self) -> &str {
        &self.locale
    }

    /// List all available locales with display-name metadata.
    pub fn available_locales() -> Vec<LocaleInfo> {
        vec![
            LocaleInfo {
                code: "en".into(),
                name: "English".into(),
                native_name: "English".into(),
            },
            LocaleInfo {
                code: "zh-cn".into(),
                name: "Chinese (Simplified)".into(),
                native_name: "简体中文".into(),
            },
            LocaleInfo {
                code: "zh-tw".into(),
                name: "Chinese (Traditional)".into(),
                native_name: "繁體中文".into(),
            },
            LocaleInfo {
                code: "ja".into(),
                name: "Japanese".into(),
                native_name: "日本語".into(),
            },
            LocaleInfo {
                code: "ko".into(),
                name: "Korean".into(),
                native_name: "한국어".into(),
            },
            LocaleInfo {
                code: "de".into(),
                name: "German".into(),
                native_name: "Deutsch".into(),
            },
            LocaleInfo {
                code: "fr".into(),
                name: "French".into(),
                native_name: "Français".into(),
            },
            LocaleInfo {
                code: "es".into(),
                name: "Spanish".into(),
                native_name: "Español".into(),
            },
            LocaleInfo {
                code: "pt-br".into(),
                name: "Portuguese (Brazil)".into(),
                native_name: "Português (Brasil)".into(),
            },
            LocaleInfo {
                code: "ru".into(),
                name: "Russian".into(),
                native_name: "Русский".into(),
            },
            LocaleInfo {
                code: "it".into(),
                name: "Italian".into(),
                native_name: "Italiano".into(),
            },
            LocaleInfo {
                code: "pl".into(),
                name: "Polish".into(),
                native_name: "Polski".into(),
            },
            LocaleInfo {
                code: "tr".into(),
                name: "Turkish".into(),
                native_name: "Türkçe".into(),
            },
            LocaleInfo {
                code: "cs".into(),
                name: "Czech".into(),
                native_name: "Čeština".into(),
            },
        ]
    }

    /// Load translations from a JSON string (`{ "key": "value", … }`).
    ///
    /// Returns the number of entries loaded. Malformed JSON returns an error.
    pub fn load_json(&mut self, json: &str) -> Result<usize, serde_json::Error> {
        let map: HashMap<String, String> = serde_json::from_str(json)?;
        let count = map.len();
        self.translations.extend(map);
        Ok(count)
    }
}

impl Default for I18n {
    fn default() -> Self {
        Self::new("en")
    }
}

// ---------------------------------------------------------------------------
// Built-in English translations
// ---------------------------------------------------------------------------

fn english_translations() -> HashMap<String, String> {
    let pairs: &[(&str, &str)] = &[
        // Menu bar
        ("menu.file", "File"),
        ("menu.edit", "Edit"),
        ("menu.selection", "Selection"),
        ("menu.view", "View"),
        ("menu.go", "Go"),
        ("menu.run", "Run"),
        ("menu.terminal", "Terminal"),
        ("menu.help", "Help"),
        // File menu
        ("menu.file.newFile", "New File"),
        ("menu.file.newWindow", "New Window"),
        ("menu.file.openFile", "Open File…"),
        ("menu.file.openFolder", "Open Folder…"),
        ("menu.file.openRecent", "Open Recent"),
        ("menu.file.save", "Save"),
        ("menu.file.saveAs", "Save As…"),
        ("menu.file.saveAll", "Save All"),
        ("menu.file.closeEditor", "Close Editor"),
        ("menu.file.closeWindow", "Close Window"),
        ("menu.file.preferences", "Preferences"),
        ("menu.file.exit", "Exit"),
        // Edit menu
        ("menu.edit.undo", "Undo"),
        ("menu.edit.redo", "Redo"),
        ("menu.edit.cut", "Cut"),
        ("menu.edit.copy", "Copy"),
        ("menu.edit.paste", "Paste"),
        ("menu.edit.find", "Find"),
        ("menu.edit.replace", "Replace"),
        ("menu.edit.findInFiles", "Find in Files"),
        ("menu.edit.replaceInFiles", "Replace in Files"),
        // View menu
        ("menu.view.commandPalette", "Command Palette…"),
        ("menu.view.explorer", "Explorer"),
        ("menu.view.search", "Search"),
        ("menu.view.scm", "Source Control"),
        ("menu.view.debug", "Run and Debug"),
        ("menu.view.extensions", "Extensions"),
        ("menu.view.terminal", "Terminal"),
        ("menu.view.output", "Output"),
        ("menu.view.problems", "Problems"),
        ("menu.view.minimap", "Minimap"),
        ("menu.view.wordWrap", "Word Wrap"),
        ("menu.view.zoomIn", "Zoom In"),
        ("menu.view.zoomOut", "Zoom Out"),
        ("menu.view.resetZoom", "Reset Zoom"),
        ("menu.view.fullScreen", "Full Screen"),
        ("menu.view.zenMode", "Zen Mode"),
        // Go menu
        ("menu.go.back", "Back"),
        ("menu.go.forward", "Forward"),
        ("menu.go.goToFile", "Go to File…"),
        ("menu.go.goToLine", "Go to Line/Column…"),
        ("menu.go.goToSymbol", "Go to Symbol in Editor…"),
        ("menu.go.goToSymbolWorkspace", "Go to Symbol in Workspace…"),
        ("menu.go.goToDefinition", "Go to Definition"),
        ("menu.go.goToReferences", "Go to References"),
        ("menu.go.goToImplementation", "Go to Implementation"),
        // Run menu
        ("menu.run.startDebugging", "Start Debugging"),
        ("menu.run.startWithoutDebugging", "Run Without Debugging"),
        ("menu.run.stopDebugging", "Stop Debugging"),
        ("menu.run.addConfiguration", "Add Configuration…"),
        // Terminal menu
        ("menu.terminal.newTerminal", "New Terminal"),
        ("menu.terminal.splitTerminal", "Split Terminal"),
        // Help menu
        ("menu.help.welcome", "Welcome"),
        ("menu.help.documentation", "Documentation"),
        ("menu.help.releaseNotes", "Release Notes"),
        ("menu.help.reportIssue", "Report Issue"),
        ("menu.help.about", "About"),
        // Status bar
        ("statusBar.line", "Ln {line}"),
        ("statusBar.column", "Col {col}"),
        ("statusBar.encoding", "UTF-8"),
        ("statusBar.lineEnding.lf", "LF"),
        ("statusBar.lineEnding.crlf", "CRLF"),
        ("statusBar.spaces", "Spaces: {size}"),
        ("statusBar.tabs", "Tab Size: {size}"),
        ("statusBar.language", "{language}"),
        ("statusBar.notifications", "Notifications"),
        ("statusBar.noProblems", "No Problems"),
        ("statusBar.problems", "{count} Problem(s)"),
        // Dialogs
        ("dialog.save.title", "Save"),
        ("dialog.save.message", "Do you want to save the changes you made to {file}?"),
        ("dialog.save.saveButton", "Save"),
        ("dialog.save.dontSaveButton", "Don't Save"),
        ("dialog.save.cancelButton", "Cancel"),
        ("dialog.confirmDelete.title", "Confirm Delete"),
        ("dialog.confirmDelete.message", "Are you sure you want to delete '{name}'?"),
        ("dialog.openFolder.title", "Open Folder"),
        ("dialog.openFile.title", "Open File"),
        // Quick open
        ("quickOpen.placeholder", "Type the name of a file to open…"),
        ("quickOpen.noResults", "No matching results"),
        // Command palette
        ("commandPalette.placeholder", "Type a command…"),
        // Welcome
        ("welcome.title", "Welcome to SideX"),
        ("welcome.start", "Start"),
        ("welcome.newFile", "New File"),
        ("welcome.openFile", "Open File…"),
        ("welcome.openFolder", "Open Folder…"),
        ("welcome.recent", "Recent"),
        ("welcome.noRecent", "No recent files or folders"),
        // Extensions
        ("extensions.search", "Search Extensions in Marketplace"),
        ("extensions.installed", "Installed"),
        ("extensions.recommended", "Recommended"),
        ("extensions.install", "Install"),
        ("extensions.uninstall", "Uninstall"),
        ("extensions.enable", "Enable"),
        ("extensions.disable", "Disable"),
        // Settings
        ("settings.title", "Settings"),
        ("settings.search", "Search settings"),
        ("settings.modified", "(Modified)"),
        // Search panel
        ("search.placeholder", "Search"),
        ("search.replaceWith", "Replace with"),
        ("search.resultsCount", "{count} results in {files} files"),
        ("search.noResults", "No results found"),
        // Problems panel
        ("problems.title", "Problems"),
        ("problems.errors", "{count} Error(s)"),
        ("problems.warnings", "{count} Warning(s)"),
        ("problems.infos", "{count} Info(s)"),
        // Output panel
        ("output.title", "Output"),
        // Debug
        ("debug.start", "Start Debugging"),
        ("debug.stop", "Stop"),
        ("debug.continue", "Continue"),
        ("debug.stepOver", "Step Over"),
        ("debug.stepInto", "Step Into"),
        ("debug.stepOut", "Step Out"),
        ("debug.restart", "Restart"),
        // Notifications
        ("notification.extensionInstalled", "Extension '{name}' installed successfully."),
        (
            "notification.extensionFailed",
            "Failed to install extension '{name}'.",
        ),
        ("notification.fileNotFound", "File not found: {path}"),
        ("notification.savedSuccessfully", "File saved successfully."),
    ];

    pairs
        .iter()
        .map(|&(k, v)| (k.to_owned(), v.to_owned()))
        .collect()
}

/// Stub translations for a non-English locale.
///
/// In a production build, these would be loaded from embedded JSON resource
/// files.  For now each locale carries a small set of translated keys so
/// the infrastructure is exercised, and the remainder fall back to English.
fn translations_for(locale: &str) -> HashMap<String, String> {
    let pairs: &[(&str, &str)] = match locale {
        "zh-cn" => &[
            ("menu.file", "文件"),
            ("menu.edit", "编辑"),
            ("menu.selection", "选择"),
            ("menu.view", "查看"),
            ("menu.go", "转到"),
            ("menu.run", "运行"),
            ("menu.terminal", "终端"),
            ("menu.help", "帮助"),
            ("menu.file.newFile", "新建文件"),
            ("menu.file.openFile", "打开文件…"),
            ("menu.file.save", "保存"),
            ("welcome.title", "欢迎使用 SideX"),
        ],
        "zh-tw" => &[
            ("menu.file", "檔案"),
            ("menu.edit", "編輯"),
            ("menu.view", "檢視"),
            ("menu.help", "說明"),
            ("menu.file.newFile", "新增檔案"),
            ("menu.file.save", "儲存"),
            ("welcome.title", "歡迎使用 SideX"),
        ],
        "ja" => &[
            ("menu.file", "ファイル"),
            ("menu.edit", "編集"),
            ("menu.view", "表示"),
            ("menu.help", "ヘルプ"),
            ("menu.file.newFile", "新規ファイル"),
            ("menu.file.save", "保存"),
            ("welcome.title", "SideX へようこそ"),
        ],
        "ko" => &[
            ("menu.file", "파일"),
            ("menu.edit", "편집"),
            ("menu.view", "보기"),
            ("menu.help", "도움말"),
            ("menu.file.newFile", "새 파일"),
            ("menu.file.save", "저장"),
            ("welcome.title", "SideX에 오신 것을 환영합니다"),
        ],
        "de" => &[
            ("menu.file", "Datei"),
            ("menu.edit", "Bearbeiten"),
            ("menu.view", "Ansicht"),
            ("menu.help", "Hilfe"),
            ("menu.file.newFile", "Neue Datei"),
            ("menu.file.save", "Speichern"),
            ("welcome.title", "Willkommen bei SideX"),
        ],
        "fr" => &[
            ("menu.file", "Fichier"),
            ("menu.edit", "Modifier"),
            ("menu.view", "Affichage"),
            ("menu.help", "Aide"),
            ("menu.file.newFile", "Nouveau fichier"),
            ("menu.file.save", "Enregistrer"),
            ("welcome.title", "Bienvenue dans SideX"),
        ],
        "es" => &[
            ("menu.file", "Archivo"),
            ("menu.edit", "Editar"),
            ("menu.view", "Ver"),
            ("menu.help", "Ayuda"),
            ("menu.file.newFile", "Nuevo archivo"),
            ("menu.file.save", "Guardar"),
            ("welcome.title", "Bienvenido a SideX"),
        ],
        "pt-br" => &[
            ("menu.file", "Arquivo"),
            ("menu.edit", "Editar"),
            ("menu.view", "Exibir"),
            ("menu.help", "Ajuda"),
            ("menu.file.newFile", "Novo Arquivo"),
            ("menu.file.save", "Salvar"),
            ("welcome.title", "Bem-vindo ao SideX"),
        ],
        "ru" => &[
            ("menu.file", "Файл"),
            ("menu.edit", "Правка"),
            ("menu.view", "Вид"),
            ("menu.help", "Справка"),
            ("menu.file.newFile", "Новый файл"),
            ("menu.file.save", "Сохранить"),
            ("welcome.title", "Добро пожаловать в SideX"),
        ],
        "it" => &[
            ("menu.file", "File"),
            ("menu.edit", "Modifica"),
            ("menu.view", "Visualizza"),
            ("menu.help", "Guida"),
            ("menu.file.newFile", "Nuovo file"),
            ("menu.file.save", "Salva"),
            ("welcome.title", "Benvenuto in SideX"),
        ],
        "pl" => &[
            ("menu.file", "Plik"),
            ("menu.edit", "Edycja"),
            ("menu.view", "Widok"),
            ("menu.help", "Pomoc"),
            ("menu.file.newFile", "Nowy plik"),
            ("menu.file.save", "Zapisz"),
            ("welcome.title", "Witamy w SideX"),
        ],
        "tr" => &[
            ("menu.file", "Dosya"),
            ("menu.edit", "Düzenle"),
            ("menu.view", "Görünüm"),
            ("menu.help", "Yardım"),
            ("menu.file.newFile", "Yeni Dosya"),
            ("menu.file.save", "Kaydet"),
            ("welcome.title", "SideX'e Hoş Geldiniz"),
        ],
        "cs" => &[
            ("menu.file", "Soubor"),
            ("menu.edit", "Úpravy"),
            ("menu.view", "Zobrazení"),
            ("menu.help", "Nápověda"),
            ("menu.file.newFile", "Nový soubor"),
            ("menu.file.save", "Uložit"),
            ("welcome.title", "Vítejte v SideX"),
        ],
        _ => &[],
    };

    pairs
        .iter()
        .map(|&(k, v)| (k.to_owned(), v.to_owned()))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn english_default() {
        let i18n = I18n::default();
        assert_eq!(i18n.locale(), "en");
        assert_eq!(i18n.t("menu.file"), "File");
    }

    #[test]
    fn missing_key_returns_key() {
        let i18n = I18n::default();
        assert_eq!(i18n.t("nonexistent.key"), "nonexistent.key");
    }

    #[test]
    fn locale_switch() {
        let mut i18n = I18n::new("en");
        i18n.set_locale("de");
        assert_eq!(i18n.locale(), "de");
        assert_eq!(i18n.t("menu.file"), "Datei");
    }

    #[test]
    fn fallback_to_english() {
        let i18n = I18n::new("de");
        assert_eq!(i18n.t("menu.file.saveAll"), "Save All");
    }

    #[test]
    fn interpolation() {
        let i18n = I18n::default();
        let result = i18n.t_with_args("statusBar.line", &[("line", "42")]);
        assert_eq!(result, "Ln 42");
    }

    #[test]
    fn multiple_interpolation_args() {
        let i18n = I18n::default();
        let result = i18n.t_with_args(
            "dialog.save.message",
            &[("file", "main.rs")],
        );
        assert_eq!(
            result,
            "Do you want to save the changes you made to main.rs?"
        );
    }

    #[test]
    fn all_locales_listed() {
        let locales = I18n::available_locales();
        assert_eq!(locales.len(), 14);
        assert_eq!(locales[0].code, "en");
        assert_eq!(locales[locales.len() - 1].code, "cs");
    }

    #[test]
    fn each_locale_loads() {
        for info in I18n::available_locales() {
            let i18n = I18n::new(&info.code);
            assert!(!i18n.t("menu.file").is_empty());
        }
    }

    #[test]
    fn load_json_extends_translations() {
        let mut i18n = I18n::new("en");
        let json = r#"{"custom.key": "Custom Value"}"#;
        let count = i18n.load_json(json).unwrap();
        assert_eq!(count, 1);
        assert_eq!(i18n.t("custom.key"), "Custom Value");
    }

    #[test]
    fn chinese_simplified() {
        let i18n = I18n::new("zh-cn");
        assert_eq!(i18n.t("menu.file"), "文件");
        assert_eq!(i18n.t("welcome.title"), "欢迎使用 SideX");
    }

    #[test]
    fn japanese() {
        let i18n = I18n::new("ja");
        assert_eq!(i18n.t("menu.file"), "ファイル");
    }
}
