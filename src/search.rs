use reqwest::blocking::Client;
use reqwest::header;
use serde_json::Value;

pub fn search(query: &str) {
    let url = format!("https://api.github.com/search/repositories?q={}",
                     urlencoding::encode(query));

    let client = Client::new();
    let response = client.get(&url)
        .header(header::USER_AGENT, "charoite-pkg-manager")
        .send();

    let resp = match response {
        Ok(resp) => resp,
        Err(e) => {
            eprintln!("Failed to access GitHub API: {}", e);
            return;
        }
    };

    if !resp.status().is_success() {
        eprintln!("GitHub API error: {} - {}",
                 resp.status(),
                 resp.text().unwrap_or_default());
        return;
    }

    let json: Value = match resp.json() {
        Ok(v) => v,
        Err(e) => {
            eprintln!("Failed to parse GitHub response: {}", e);
            return;
        }
    };

    if let Some(items) = json["items"].as_array() {
        println!("{:<40} {:<8} {:<8} {}", "Package", "Stars", "Forks", "Source");
        println!("{}", "-".repeat(70));

        for item in items.iter().take(10) {
            if let Some(name) = item["full_name"].as_str() {
                let stars = item["stargazers_count"].as_u64().unwrap_or(0);
                let forks = item["forks_count"].as_u64().unwrap_or(0);
                
                println!("{:<40} {:<8} {:<8} GitHub", name, stars, forks);
            }
        }
    } else {
        eprintln!("Unexpected GitHub API response format");
        if let Some(message) = json["message"].as_str() {
            eprintln!("GitHub says: {}", message);
        }
    }
}
