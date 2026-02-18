use std::collections::HashMap;

/// In-memory store of open document text buffers.
pub struct DocumentStore {
    docs: HashMap<String, String>,
}

impl DocumentStore {
    pub fn new() -> Self {
        Self {
            docs: HashMap::new(),
        }
    }

    pub fn open(&mut self, uri: &str, text: String) {
        self.docs.insert(uri.to_string(), text);
    }

    pub fn change(&mut self, uri: &str, text: String) {
        self.docs.insert(uri.to_string(), text);
    }

    pub fn close(&mut self, uri: &str) {
        self.docs.remove(uri);
    }

    pub fn get(&self, uri: &str) -> Option<&str> {
        self.docs.get(uri).map(|s| s.as_str())
    }

    pub fn uris(&self) -> impl Iterator<Item = &String> {
        self.docs.keys()
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
