// The modules `generate` and `parse` now live in `src/lib.rs` so they
// can also be exercised from integration tests; pull what the CLI
// needs back into scope from the library crate.
use openapi_parser::generate::*;

use clap::{Parser, ValueEnum};

#[derive(Clone, Copy, ValueEnum)]
enum DestinationLanguage {
    Dart,
}

impl Generator for DestinationLanguage {
    async fn generate(&self, spec: &oas3::Spec, args: GenerationArgs) -> Result<Vec<File>, String> {
        let generator = match self {
            DestinationLanguage::Dart => DartGenerator,
        };
        generator.generate(spec, args).await
    }
}

impl std::fmt::Display for DestinationLanguage {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let name = match self {
            DestinationLanguage::Dart => "dart",
        };
        write!(f, "{}", name)
    }
}

async fn fetch_spec_json(url: &str) -> Result<String, reqwest::Error> {
    let fake = false;
    if fake {
        return Ok(include_str!("../test_scheme.json").to_string());
    }
    let username = std::env::var("SWAGGER_BASIC_USER");
    let password = std::env::var("SWAGGER_BASIC_PASS");

    let client = reqwest::Client::builder()
        .user_agent("curl/8.7.1")
        .build()
        .unwrap();

    let response = match username {
        Ok(username) => client.get(url).basic_auth(username, password.ok()).send(),
        Err(_) => client.get(url).send(),
    }
    .await?;
    let body = response.text().await?;
    Ok(body)
}

/// Generate client code from an OpenAPI spec.
#[derive(Parser)]
#[command(version, about)]
struct Cli {
    /// URL of the OpenAPI spec to fetch.
    #[arg(long, short = 'u')]
    spec_url: String,

    /// Directory to write the generated files into.
    #[arg(long, short = 'o', default_value = "out")]
    out_dir: std::path::PathBuf,

    /// Target language for the generated code.
    #[arg(long, short = 'd', default_value = "dart")]
    destination_language: DestinationLanguage,

    /// Keep properties/parameters that reference deprecated schemes.
    #[arg(long)]
    skip_deprecated_removal: bool,

    /// Emit immutable/unmodifiable types: `final` properties + `const`
    /// constructors (eager) and a private `_rawValue` (lazy).
    #[arg(long, default_value_t = true, action = clap::ArgAction::Set)]
    unmodifiable: bool,

    /// Emit map-backed lazy models (properties parse on read) instead of
    /// eager typed models that parse in `fromJson`.
    #[arg(long, default_value_t = false, action = clap::ArgAction::Set)]
    lazy: bool,
}

#[tokio::main]
async fn main() {
    let cli = Cli::parse();

    let spec_url = cli.spec_url;
    let out_dir = cli.out_dir;
    let destination_language = cli.destination_language;
    let skip_deprecated_removal = cli.skip_deprecated_removal;
    let unmodifiable = cli.unmodifiable;
    let lazy = cli.lazy;

    println!("getting spec");
    let json = match fetch_spec_json(&spec_url).await {
        Ok(json) => json,
        Err(e) => {
            println!("fetching spec error: {:?}", e);
            return;
        }
    };
    println!("parsing spec");
    let spec = match oas3::from_json(json) {
        Ok(spec) => spec,
        Err(e) => {
            println!("parsing spec error: {:?}", e);
            return;
        }
    };
    println!("generating {:} code", destination_language);
    let files = match destination_language
        .generate(
            &spec,
            GenerationArgs {
                ignore_deprecated_fields: !skip_deprecated_removal,
                unmodifiable,
                lazy,
            },
        )
        .await
    {
        Ok(files) => files,
        Err(e) => {
            println!("generating code error: {:?}", e);
            return;
        }
    };
    if files.is_empty() {
        println!("no files to write");
        return;
    }
    print!("writing files");
    // start from scratch, i.e. rm -rf out_dir
    let _ = std::fs::remove_dir_all(&out_dir);
    print!(":");
    for file in files {
        let path = out_dir.join(&file.path);
        // Create parent directories if they don't exist
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).unwrap();
        }
        std::fs::write(path, file.content).unwrap();
        print!(".");
    }
    println!("");
    println!("done");

    // exit with 0
    std::process::exit(0);
}
