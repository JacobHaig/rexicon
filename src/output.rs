use anyhow::Result;
use std::path::Path;

pub fn write_output(text: &str, path: &Path) -> Result<()> {
    std::fs::write(path, text)?;
    Ok(())
}
