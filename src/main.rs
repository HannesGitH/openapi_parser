mod generate;
use generate::*;

mod parse;

enum DestinationLanguage {
    Dart,
}

impl Generator for DestinationLanguage {
    async fn generate(&self, spec: &oas3::Spec) -> Result<Vec<File>, String> {
        let generator = match self {
            DestinationLanguage::Dart => DartGenerator,
        };
        generator.generate(spec).await
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

    let client = reqwest::Client::new();

    let response = match username {
        Ok(username) => client
            .get(url)
            .basic_auth(username, password.ok())
            .send(),
        Err(_) => client.get(url).send(),
    }.await?;
    let body = response.text().await?;
    Ok(body)
}

#[tokio::main]
async fn main() {

    // parse args from the following format
    // --spec-url/-u <spec-url> --out-dir/-o <out-dir> --destination-language/-d <destination-language> 
    let mut args = std::env::args().skip(1);
    let mut spec_url = None;
    let mut out_dir = None;
    let mut destination_language = None;

    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--spec-url" | "-u" => {
                spec_url = args.next();
            }
            "--out-dir" | "-o" => {
                out_dir = args.next().map(std::path::PathBuf::from);
            }
            "--destination-language" | "-d" => {
                destination_language = args.next().map(|lang| match lang.as_str() {
                    "dart" => DestinationLanguage::Dart,
                    _ => panic!("Unsupported destination language")
                });
            }
            _ => panic!("Unknown argument: {}", arg)
        }
    }

    let spec_url = spec_url.unwrap_or_else(|| {
        panic!("Missing required argument: --spec-url/-u")
    });

    let out_dir = out_dir.unwrap_or_else(|| std::path::PathBuf::from("out"));

    let destination_language = destination_language.unwrap_or(DestinationLanguage::Dart);

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
    let files = match destination_language.generate(&spec).await {
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
