use std::collections::HashMap;

/// Document entry with text and version counter.
struct DocEntry {
    text: String,
    version: u64,
}

/// In-memory store of open document text buffers with version tracking.
pub struct DocumentStore {
    docs: HashMap<String, DocEntry>,
}

impl DocumentStore {
    pub fn new() -> Self {
        Self {
            docs: HashMap::new(),
        }
    }

    pub fn open(&mut self, uri: &str, text: String) {
        self.docs.insert(uri.to_string(), DocEntry { text, version: 1 });
    }

    pub fn change(&mut self, uri: &str, text: String) {
        let version = self.docs.get(uri).map_or(1, |e| e.version + 1);
        self.docs.insert(uri.to_string(), DocEntry { text, version });
    }

    pub fn close(&mut self, uri: &str) {
        self.docs.remove(uri);
    }

    pub fn get(&self, uri: &str) -> Option<&str> {
        self.docs.get(uri).map(|e| e.text.as_str())
    }

    pub fn version(&self, uri: &str) -> u64 {
        self.docs.get(uri).map_or(0, |e| e.version)
    }

    pub fn uris(&self) -> impl Iterator<Item = &String> {
        self.docs.keys()
    }

    pub fn is_open(&self, uri: &str) -> bool {
        self.docs.contains_key(uri)
    }
}

/// Convert a file:// URI to a filesystem path.
pub fn uri_to_path(uri: &str) -> String {
    if let Some(path) = uri.strip_prefix("file://") {
        // URL-decode basic percent-encoding
        let mut result = String::new();
        let mut chars = path.chars();
        while let Some(ch) = chars.next() {
            if ch == '%' {
                let mut hex = String::new();
                if let Some(h1) = chars.next() { hex.push(h1); }
                if let Some(h2) = chars.next() { hex.push(h2); }
                if let Ok(byte) = u8::from_str_radix(&hex, 16) {
                    result.push(byte as char);
                } else {
                    result.push('%');
                    result.push_str(&hex);
                }
            } else {
                result.push(ch);
            }
        }
        result
    } else {
        uri.to_string()
    }
}

/// Convert a filesystem path to a file:// URI.
pub fn path_to_uri(path: &str) -> String {
    format!("file://{}", path)
}
