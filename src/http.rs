use reqwest::Method;
use reqwest::blocking::Client;
use reqwest::header::{HeaderMap, HeaderName, HeaderValue};

use crate::cli::Command;
use crate::config::Config;
use crate::error::AwError;
use crate::{mpp, payment, wallet};

pub fn execute_request(command: &Command, config: &Config) -> Result<(), AwError> {
    let (method, url, body, raw_headers) = extract_request_parts(command);
    let headers = parse_headers(&raw_headers)?;

    let client = Client::builder()
        .timeout(std::time::Duration::from_secs(30))
        .build()
        .map_err(|e| AwError::Config(format!("failed to build HTTP client: {e}")))?;

    // Initial request
    let mut req = client
        .request(method.clone(), &url)
        .headers(headers.clone());
    if let Some(ref b) = body {
        req = req.body(b.clone());
    }
    let response = req.send().map_err(|e| AwError::Http {
        status: 0,
        url: url.clone(),
        body: e.to_string(),
    })?;

    // Non-402: handle normally
    if response.status().as_u16() != 402 {
        let status = response.status();
        let body = response.text().unwrap_or_default();
        print!("{body}");
        if status.is_success() {
            return Ok(());
        } else {
            return Err(AwError::Http {
                status: status.as_u16(),
                url,
                body,
            });
        }
    }

    // 402: parse challenge
    let www_auth = response
        .headers()
        .get("www-authenticate")
        .ok_or_else(|| AwError::Mpp("402 response without WWW-Authenticate header".into()))?
        .to_str()
        .map_err(|e| AwError::Mpp(format!("invalid WWW-Authenticate header encoding: {e}")))?;

    let challenge = mpp::parse_challenge(www_auth)?;
    let payment_request = mpp::decode_request(&challenge.request)?;

    // Budget check
    let max_cost = config.max_cost.ok_or_else(|| {
        AwError::Config(
            "402 received but no --max-cost set\n  → set AW_MAX_COST or pass --max-cost".into(),
        )
    })?;
    let amount_human = mpp::check_budget(&payment_request, max_cost)?;

    // Dry run
    if config.dry_run {
        if config.json_output {
            eprintln!(
                r#"{{"dry_run":true,"cost":{amount_human},"currency":"{}","recipient":"{}","description":{}}}"#,
                payment_request.currency,
                payment_request.recipient,
                challenge
                    .description
                    .as_ref()
                    .map_or("null".to_string(), |d| format!(r#""{d}""#)),
            );
        } else {
            eprintln!(
                "dry run: would pay {amount_human} {}",
                payment_request.currency
            );
            eprintln!("  recipient: {}", payment_request.recipient);
            if let Some(ref desc) = challenge.description {
                eprintln!("  description: {desc}");
            }
        }
        return Ok(());
    }

    // Execute payment
    let keypair = wallet::load_keypair(&config.keypair_path)?;
    let signature =
        payment::send_payment(&keypair, &payment_request, &challenge.id, &config.rpc_url)?;

    // Build credential and retry
    let auth_header = mpp::build_authorization_header(&challenge, &signature);
    let mut retry_req = client
        .request(method, &url)
        .headers(headers)
        .header("Authorization", &auth_header);
    if let Some(ref b) = body {
        retry_req = retry_req.body(b.clone());
    }

    let retry_response = retry_req.send().map_err(|e| AwError::Http {
        status: 0,
        url: url.clone(),
        body: e.to_string(),
    })?;

    // Emit spend line to stderr
    eprintln!(
        r#"{{"paid":{amount_human},"currency":"{}","recipient":"{}","signature":"{signature}"}}"#,
        payment_request.currency, payment_request.recipient,
    );

    // Print response body to stdout
    let status = retry_response.status();
    let response_body = retry_response.text().unwrap_or_default();
    print!("{response_body}");

    if status.is_success() {
        Ok(())
    } else {
        Err(AwError::Http {
            status: status.as_u16(),
            url,
            body: response_body,
        })
    }
}

fn extract_request_parts(command: &Command) -> (Method, String, Option<String>, Vec<String>) {
    match command {
        Command::Get { url, header } => (Method::GET, url.clone(), None, header.clone()),
        Command::Post { url, body, header } => {
            (Method::POST, url.clone(), body.clone(), header.clone())
        }
        Command::Put { url, body, header } => {
            (Method::PUT, url.clone(), body.clone(), header.clone())
        }
        Command::Delete { url, header } => (Method::DELETE, url.clone(), None, header.clone()),
        _ => unreachable!("execute_request called with non-HTTP command"),
    }
}

fn parse_headers(raw: &[String]) -> Result<HeaderMap, AwError> {
    let mut map = HeaderMap::new();
    for h in raw {
        let (key, value) = h.split_once(':').ok_or_else(|| {
            AwError::Config(format!("invalid header (expected 'Key: Value'): {h}"))
        })?;
        let name = HeaderName::from_bytes(key.trim().as_bytes())
            .map_err(|e| AwError::Config(format!("invalid header name '{key}': {e}")))?;
        let val = HeaderValue::from_str(value.trim())
            .map_err(|e| AwError::Config(format!("invalid header value for '{key}': {e}")))?;
        map.insert(name, val);
    }
    Ok(map)
}
