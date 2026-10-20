//! A generic settings CLI over any app's settings folder:
//! `app-settings --dir <folder> --manifest <schema-and-commands.json> [--version v] [--app-name Name] <verb> …`.
//! The manifest is `{ "schema": <defineSchema input>, "commands": <registry defs> }`, as an app's TS build emits it.
//! Apps normally expose the same verbs through their own binary (`<app> settings …`); this one is for tests and for apps without one.

use app_settings::{run_process, CliOptions, Registry, Schema};
use std::sync::Arc;

fn main() {
    let mut args: Vec<String> = std::env::args().skip(1).collect();
    let mut take = |name: &str| -> Option<String> {
        let i = args.iter().position(|a| a == name)?;
        let v = args.get(i + 1).cloned();
        args.drain(i..(i + 2).min(args.len()));
        v
    };
    let (Some(dir), Some(manifest)) = (take("--dir"), take("--manifest")) else {
        eprintln!("Usage: app-settings --dir <folder> --manifest <file.json> [--version v] [--app-name Name] <verb> …");
        std::process::exit(2);
    };
    let version = take("--version").unwrap_or_else(|| "0.0.0".into());
    let app_name = take("--app-name");
    let text = std::fs::read_to_string(&manifest).unwrap_or_default();
    let m: serde_json::Value = serde_json::from_str(&text).unwrap_or_default();
    let schema = match Schema::from_json(&m["schema"]) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("manifest schema: {e}");
            std::process::exit(2);
        }
    };
    let commands = Registry::from_json(&m["commands"]).unwrap_or_else(|_| Registry::new(vec![]));
    let opts = CliOptions {
        dir: dir.into(),
        app_name: app_name.unwrap_or_else(|| schema.app.clone()),
        schema: Arc::new(schema),
        commands: Arc::new(commands),
        version,
        migrate: None,
        root: None,
    };
    std::process::exit(run_process(&args, &opts));
}
