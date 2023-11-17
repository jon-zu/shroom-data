use std::io::Write;
use std::path::Path;
use std::sync::Arc;
use std::{fs::File, io};

use jsonschema::{Draft, ErrorIterator, JSONSchema, SchemaResolver};
use rayon::prelude::{ParallelBridge, ParallelIterator};
use url::Url;

#[derive(Debug, serde::Deserialize)]
pub struct Schema {
    #[serde(rename = "fileMatch")]
    pub file_match: Vec<String>,
    pub url: String,
}

fn report_error<'a>(file: &'a Path, errs: ErrorIterator<'a>) {
    let mut stdout = io::stdout().lock();
    for err in errs {
        writeln!(&mut stdout, "{file:?} - {err}").unwrap();
    }
}

pub struct Resolver(Arc<serde_json::Value>);

impl SchemaResolver for Resolver {
    fn resolve(
        &self,
        _root_schema: &serde_json::Value,
        _url: &Url,
        _original_reference: &str,
    ) -> Result<Arc<serde_json::Value>, jsonschema::SchemaResolverError> {
        Ok(self.0.clone())
    }
}

fn load_json(path: impl AsRef<Path>) -> anyhow::Result<serde_json::Value> {
    let file = File::open(path)?;
    let json: serde_json::Value = serde_json::from_reader(file)?;
    Ok(json)
}

fn main() -> anyhow::Result<()> {
    let filter = std::env::args().nth(1);

    let settings = ".vscode/settings.json";
    let settings = load_json(settings)?;
    let schemas = settings["json.schemas"].as_array().unwrap();
    let schemas = schemas
        .iter()
        .map(|v| serde_json::from_value::<Schema>(v.clone()))
        .collect::<Result<Vec<_>, serde_json::Error>>()?;

    let shroom_schema = load_json("./schemas/shroom.schema.json")?;
    let shroom_schema = Arc::new(shroom_schema);

    for schema_match in schemas.iter() {
        if let Some(filter) = &filter {
            if !schema_match.url.contains(filter) {
                continue;
            }
        }
        println!("Checking {}", schema_match.url);
        let schema = load_json(&schema_match.url)?;

        let resolver = Resolver(shroom_schema.clone());
        let compiled = JSONSchema::options()
            .with_draft(Draft::Draft7)
            .with_resolver(resolver)
            .compile(&schema)
            .map_err(|_| anyhow::anyhow!("Failed to compile schema"))?;
        let compiled = Arc::new(compiled);

        glob::glob(&schema_match.file_match[0])?
            .par_bridge()
            .for_each(|file| {
                let file = file.unwrap();
                let json = load_json(&file).unwrap();
                if let Err(errs) = compiled.validate(&json) {
                    report_error(file.as_path(), errs);
                };
            });
    }

    Ok(())
}
