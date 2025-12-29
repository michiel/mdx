//! Image handling for Markdown documents

use std::path::{Path, PathBuf};

/// Represents an image in the Markdown document
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ImageNode {
    /// Image source (path or URL)
    pub src: String,
    /// Alt text
    pub alt: String,
    /// Optional title
    pub title: Option<String>,
    /// Source line number in document
    pub source_line: usize,
}

/// Image resolution result
#[derive(Debug, Clone)]
pub enum ImageSource {
    /// Local file path (absolute)
    Local(PathBuf),
    /// Remote URL
    Remote(String),
}

impl ImageNode {
    /// Create a new image node
    pub fn new(src: String, alt: String, source_line: usize) -> Self {
        Self {
            src,
            alt,
            title: None,
            source_line,
        }
    }

    /// Create a new image node with title
    pub fn with_title(src: String, alt: String, title: String, source_line: usize) -> Self {
        Self {
            src,
            alt,
            title: Some(title),
            source_line,
        }
    }

    /// Resolve image source relative to document path
    pub fn resolve(&self, doc_path: &Path) -> Option<ImageSource> {
        self.resolve_with_policy(doc_path, true, true)
    }

    /// Resolve image source relative to document path with policy controls
    pub fn resolve_with_policy(
        &self,
        doc_path: &Path,
        allow_absolute: bool,
        allow_remote: bool,
    ) -> Option<ImageSource> {
        // Check if src is a URL
        if self.src.starts_with("http://") || self.src.starts_with("https://") {
            if allow_remote {
                return Some(ImageSource::Remote(self.src.clone()));
            }
            return None;
        }

        // Reject path traversal attempts before canonicalization
        if self.src.contains("..") {
            return None;
        }

        let src_path = Path::new(&self.src);
        if src_path.is_absolute() {
            if allow_absolute {
                if let Ok(canonical) = src_path.canonicalize() {
                    return Some(ImageSource::Local(canonical));
                }
            }
            return None;
        }

        // Resolve relative to document directory
        let doc_dir = doc_path.parent()?;
        let img_path = doc_dir.join(&self.src);

        // Canonicalise if it exists
        if let Ok(canonical) = img_path.canonicalize() {
            if !allow_absolute {
                if let Ok(canonical_doc_dir) = doc_dir.canonicalize() {
                    if !canonical.starts_with(canonical_doc_dir) {
                        return None;
                    }
                }
            }

            Some(ImageSource::Local(canonical))
        } else {
            None
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    #[test]
    fn test_image_node_creation() {
        let img = ImageNode::new(
            "test.png".to_string(),
            "Test image".to_string(),
            10,
        );
        assert_eq!(img.src, "test.png");
        assert_eq!(img.alt, "Test image");
        assert_eq!(img.title, None);
        assert_eq!(img.source_line, 10);
    }

    #[test]
    fn test_image_node_with_title() {
        let img = ImageNode::with_title(
            "test.png".to_string(),
            "Test image".to_string(),
            "Title".to_string(),
            10,
        );
        assert_eq!(img.title, Some("Title".to_string()));
    }

    #[test]
    fn test_resolve_http_url() {
        let img = ImageNode::new(
            "http://example.com/image.png".to_string(),
            "Remote".to_string(),
            0,
        );
        let doc_path = Path::new("/tmp/test.md");
        let resolved = img.resolve(doc_path);

        assert!(matches!(resolved, Some(ImageSource::Remote(_))));
        if let Some(ImageSource::Remote(url)) = resolved {
            assert_eq!(url, "http://example.com/image.png");
        }
    }

    #[test]
    fn test_resolve_https_url() {
        let img = ImageNode::new(
            "https://example.com/image.png".to_string(),
            "Remote".to_string(),
            0,
        );
        let doc_path = Path::new("/tmp/test.md");
        let resolved = img.resolve(doc_path);

        assert!(matches!(resolved, Some(ImageSource::Remote(_))));
    }

    #[test]
    fn test_resolve_local_path() {
        let temp_dir = TempDir::new().unwrap();
        let doc_path = temp_dir.path().join("test.md");
        let img_path = temp_dir.path().join("image.png");

        // Create the image file
        fs::write(&img_path, b"fake image").unwrap();

        let img = ImageNode::new("image.png".to_string(), "Local".to_string(), 0);
        let resolved = img.resolve(&doc_path);

        assert!(matches!(resolved, Some(ImageSource::Local(_))));
        if let Some(ImageSource::Local(path)) = resolved {
            assert!(path.ends_with("image.png"));
            assert!(path.is_absolute());
        }
    }

    #[test]
    fn test_resolve_relative_path() {
        let temp_dir = TempDir::new().unwrap();
        let subdir = temp_dir.path().join("images");
        fs::create_dir(&subdir).unwrap();

        let doc_path = temp_dir.path().join("test.md");
        let img_path = subdir.join("test.png");
        fs::write(&img_path, b"fake image").unwrap();

        let img = ImageNode::new("images/test.png".to_string(), "Relative".to_string(), 0);
        let resolved = img.resolve(&doc_path);

        assert!(matches!(resolved, Some(ImageSource::Local(_))));
    }

    #[test]
    fn test_resolve_nonexistent_file() {
        let temp_dir = TempDir::new().unwrap();
        let doc_path = temp_dir.path().join("test.md");

        let img = ImageNode::new("nonexistent.png".to_string(), "Missing".to_string(), 0);
        let resolved = img.resolve(&doc_path);

        assert!(resolved.is_none());
    }

    #[test]
    fn test_resolve_absolute_path() {
        let temp_dir = TempDir::new().unwrap();
        let img_path = temp_dir.path().join("image.png");
        fs::write(&img_path, b"fake image").unwrap();

        let doc_path = Path::new("/tmp/test.md");
        let img = ImageNode::new(
            img_path.to_string_lossy().to_string(),
            "Absolute".to_string(),
            0,
        );
        let resolved = img.resolve(doc_path);

        assert!(matches!(resolved, Some(ImageSource::Local(_))));
    }

    #[test]
    fn security_rejects_remote_images_by_default() {
        let img = ImageNode::new(
            "https://example.com/image.png".to_string(),
            "Remote".to_string(),
            0,
        );
        let doc_path = Path::new("/tmp/test.md");
        let resolved = img.resolve_with_policy(doc_path, false, false);

        assert!(resolved.is_none());
    }

    #[test]
    fn security_rejects_absolute_paths_by_default() {
        let temp_dir = TempDir::new().unwrap();
        let img_path = temp_dir.path().join("image.png");
        fs::write(&img_path, b"fake image").unwrap();

        let doc_path = temp_dir.path().join("test.md");
        let img = ImageNode::new(
            img_path.to_string_lossy().to_string(),
            "Absolute".to_string(),
            0,
        );
        let resolved = img.resolve_with_policy(&doc_path, false, false);

        assert!(resolved.is_none());
    }
}
