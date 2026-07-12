use anyhow::Context;
use serde_json::Value;

#[derive(Debug, Clone)]
pub struct LocaleCatalog {
    root: Value,
}

impl LocaleCatalog {
    pub fn load(code: &str) -> anyhow::Result<Self> {
        let code = if code == "zh-CN" { "zh-CN" } else { "en" };
        let bytes = crate::locales::read_locale(code)?;
        let root = serde_json::from_slice(&bytes).context("parsing native Linux locale catalog")?;
        Ok(Self { root })
    }

    pub fn text(&self, key: &str) -> String {
        self.lookup("linux", key)
            .or_else(|| self.lookup("macos", key))
            .or_else(|| self.lookup("reviewWeb", key))
            .unwrap_or(key)
            .to_string()
    }

    pub fn format(&self, key: &str, values: &[(&str, String)]) -> String {
        let mut text = self.text(key);
        for (name, value) in values {
            text = text.replace(&format!("{{{name}}}"), value);
        }
        text
    }

    fn lookup(&self, section: &str, key: &str) -> Option<&str> {
        self.root.get(section)?.get(key)?.as_str()
    }
}

#[cfg(test)]
mod tests {
    use super::LocaleCatalog;

    #[test]
    fn both_linux_catalogs_have_native_and_shared_strings() {
        for code in ["en", "zh-CN"] {
            let catalog = LocaleCatalog::load(code).unwrap();
            assert_ne!(catalog.text("neonOption"), "neonOption");
            assert_ne!(catalog.text("appTitle"), "appTitle");
        }
    }
}
