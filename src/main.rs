mod generate;
use generate::*;



enum DestinationLanguage {
    Dart,
}

impl Generator for DestinationLanguage {
    fn generate(&self, spec: &oas3::Spec) -> Vec<(std::path::PathBuf, String)> {
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

async fn fetch_spec_json() -> Result<String, reqwest::Error> {
    let url = "https://api.dev.blingcard.app/openapi?openapiSecret=a1baba99-9ce8-4578-a1a3-704b9cfad928";
    let response = reqwest::get(url).await?;
    let body = response.text().await?;
    Ok(body)
}

#[tokio::main]
async fn main() {

    //TODO: get destination language from args
    let destination_language = DestinationLanguage::Dart;

    println!("getting spec");
    let json = match fetch_spec_json().await {
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
    let files = destination_language.generate(&spec);
    if files.is_empty() {
        println!("no files to write");
        return;
    }
    println!("writing files");
    for file in files {
        println!("writing {:}", file.0.display());
        std::fs::write(file.0, file.1).unwrap();
    }
}
