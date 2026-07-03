use anyhow::Result;
use scout_archive::markdown::{self, ContainerMeta, RecordMeta};
use std::collections::HashMap;
use std::fs;
use std::path::Path;

use crate::models::{Highlight, Work};

struct ArchiveWork<'a>(&'a Work);
struct ArchiveHighlight<'a>(&'a Highlight);

impl ContainerMeta for ArchiveWork<'_> {
    fn slug(&self) -> &str {
        &self.0.slug
    }

    fn id(&self) -> &str {
        &self.0.id
    }

    fn title(&self) -> &str {
        &self.0.title
    }

    fn author(&self) -> Option<&str> {
        self.0.author.as_deref()
    }

    fn kind(&self) -> &str {
        &self.0.kind
    }

    fn source_system(&self) -> &str {
        &self.0.source_system
    }

    fn source_id(&self) -> Option<&str> {
        self.0.source_id.as_deref()
    }

    fn url(&self) -> Option<&str> {
        self.0.url.as_deref()
    }

    fn imported_at(&self) -> &str {
        &self.0.imported_at
    }

    fn updated_at(&self) -> &str {
        &self.0.updated_at
    }

    fn source_data_json(&self) -> String {
        serde_json::to_string(&self.0.source_data).unwrap_or_else(|_| "{}".into())
    }
}

impl RecordMeta for ArchiveHighlight<'_> {
    fn id(&self) -> &str {
        &self.0.id
    }

    fn text(&self) -> &str {
        &self.0.text
    }

    fn note(&self) -> Option<&str> {
        self.0.note.as_deref()
    }

    fn created_at(&self) -> Option<&str> {
        self.0.created_at.as_deref()
    }

    fn tags(&self) -> &[String] {
        &self.0.tags
    }

    fn annotation_color(&self) -> Option<&str> {
        self.0.annotation_color.as_deref()
    }

    fn annotation_type(&self) -> Option<&str> {
        self.0.annotation_type.as_deref()
    }

    fn format(&self) -> &str {
        &self.0.format
    }
}

pub fn write_archive(
    archive_path: &str,
    works: &[Work],
    highlights_by_work: &HashMap<String, Vec<&Highlight>>,
) -> Result<()> {
    let base = Path::new(archive_path);
    fs::create_dir_all(base.join("readings").join("works"))?;
    fs::create_dir_all(base.join("readings").join("fulltext"))?;
    fs::create_dir_all(base.join("readings").join("assets"))?;

    let works_dir = base.join("readings").join("works");
    for work in works {
        let file_path = works_dir.join(format!("{}.md", work.slug));
        let empty = vec![];
        let source_records = highlights_by_work.get(&work.id).unwrap_or(&empty);
        let wrapped: Vec<ArchiveHighlight<'_>> =
            source_records.iter().map(|h| ArchiveHighlight(h)).collect();
        let refs: Vec<&ArchiveHighlight<'_>> = wrapped.iter().collect();
        let content = markdown::render_container_file(&ArchiveWork(work), &refs);
        fs::write(file_path, content)?;
    }

    Ok(())
}
