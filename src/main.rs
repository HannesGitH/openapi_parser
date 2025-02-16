mod generate;
use generate::*;

mod parse;

enum DestinationLanguage {
    Dart,
}

impl Generator for DestinationLanguage {
    fn generate(&self, spec: &oas3::Spec) -> Result<Vec<File>, String> {
        let generator = match self {
            DestinationLanguage::Dart => DartGenerator,
        };
        generator.generate(spec)
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
    // let response = reqwest::get(url).await?;
    // let body = response.text().await?;
    // Ok(body)
    let body = std::fs::read_to_string("openapi.json").unwrap();
    Ok(body)
}

#[tokio::main]
async fn main() {

    //TODO: get spec url from args
    let spec_url = "https://api.dev.blingcard.app/openapi?openapiSecret=a1baba99-9ce8-4578-a1a3-704b9cfad928";

    //TODO: get dest out-dir from args
    let out_dir = std::path::PathBuf::from("out");

    //TODO: get destination language from args
    let destination_language = DestinationLanguage::Dart;

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
    let files = match destination_language.generate(&spec) {
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
    println!("writing files");
    // start from scratch, i.e. rm -rf out_dir
    let _ = std::fs::remove_dir_all(&out_dir);
    for file in files {
        let path = out_dir.join(&file.path);
        // Create parent directories if they don't exist
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).unwrap();
        }
        println!("writing {:}", path.display());
        std::fs::write(path, file.content).unwrap();
    }
}
