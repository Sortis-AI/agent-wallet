use base64::Engine;
use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use serde::{Deserialize, Serialize};

use crate::error::AwError;

// --- Types ---

/// Parsed from WWW-Authenticate: Payment ... header
#[derive(Debug, Clone)]
pub struct Challenge {
    pub id: String,
    pub realm: String,
    pub method: String,
    pub intent: String,
    pub request: String, // raw base64url — echo back as-is
    pub expires: Option<String>,
    pub description: Option<String>,
    pub digest: Option<String>,
    pub opaque: Option<String>,
}

/// Decoded from the base64url request field
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PaymentRequest {
    pub amount: String,
    pub currency: String,
    pub recipient: String,
    pub method_details: MethodDetails,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
#[allow(dead_code)] // Protocol fields — deserialized from wire, used for splits support
pub struct MethodDetails {
    pub network: String,
    pub decimals: u8,
    pub token_program: Option<String>,
    pub recent_blockhash: Option<String>,
    pub fee_payer: Option<bool>,
    pub fee_payer_key: Option<String>,
    pub splits: Option<Vec<Split>>,
}

#[derive(Debug, Deserialize)]
#[allow(dead_code)] // Protocol fields — deserialized from wire
pub struct Split {
    pub recipient: String,
    pub amount: String,
}

/// Challenge fields echoed in the credential (no description — not part of HMAC)
#[derive(Serialize)]
struct CredentialChallenge {
    id: String,
    realm: String,
    method: String,
    intent: String,
    request: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    expires: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    digest: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    opaque: Option<String>,
}

#[derive(Serialize)]
struct Credential {
    challenge: CredentialChallenge,
    payload: Payload,
}

#[derive(Serialize)]
struct Payload {
    r#type: String,
    signature: String,
}

// --- Challenge Parsing ---

/// Parse a WWW-Authenticate header value into a Challenge.
///
/// Expected format: `Payment id="...", realm="...", method="...", intent="...", request="..."`
pub fn parse_challenge(header_value: &str) -> Result<Challenge, AwError> {
    let rest = header_value
        .strip_prefix("Payment ")
        .ok_or_else(|| AwError::Mpp("WWW-Authenticate header missing Payment scheme".into()))?;

    let params = parse_params(rest)?;

    let get = |key: &str| -> Result<String, AwError> {
        params
            .iter()
            .find(|(k, _)| k == key)
            .map(|(_, v)| v.clone())
            .ok_or_else(|| AwError::Mpp(format!("missing required field: {key}")))
    };

    let get_opt = |key: &str| -> Option<String> {
        params
            .iter()
            .find(|(k, _)| k == key)
            .map(|(_, v)| v.clone())
    };

    Ok(Challenge {
        id: get("id")?,
        realm: get("realm")?,
        method: get("method")?,
        intent: get("intent")?,
        request: get("request")?,
        expires: get_opt("expires"),
        description: get_opt("description"),
        digest: get_opt("digest"),
        opaque: get_opt("opaque"),
    })
}

/// State-machine parser for key="value" pairs separated by ", "
fn parse_params(input: &str) -> Result<Vec<(String, String)>, AwError> {
    let mut params = Vec::new();
    let mut chars = input.chars().peekable();

    loop {
        // Skip whitespace
        while chars.peek().is_some_and(|c| c.is_whitespace()) {
            chars.next();
        }

        if chars.peek().is_none() {
            break;
        }

        // Read key (up to '=')
        let mut key = String::new();
        loop {
            match chars.next() {
                Some('=') => break,
                Some(c) => key.push(c),
                None => {
                    return Err(AwError::Mpp(format!(
                        "unexpected end of header after key '{key}'"
                    )));
                }
            }
        }

        // Read value
        let value = match chars.peek() {
            Some('"') => {
                chars.next(); // consume opening quote
                let mut val = String::new();
                loop {
                    match chars.next() {
                        Some('\\') => {
                            // Escaped character
                            if let Some(c) = chars.next() {
                                val.push(c);
                            }
                        }
                        Some('"') => break,
                        Some(c) => val.push(c),
                        None => return Err(AwError::Mpp("unterminated quoted value".into())),
                    }
                }
                val
            }
            _ => {
                // Unquoted value (up to comma or end)
                let mut val = String::new();
                while chars.peek().is_some_and(|&c| c != ',') {
                    val.push(chars.next().unwrap());
                }
                val
            }
        };

        params.push((key.trim().to_string(), value));

        // Consume separator (", ")
        match chars.peek() {
            Some(',') => {
                chars.next();
                // Skip optional whitespace after comma
                while chars.peek().is_some_and(|c| c.is_whitespace()) {
                    chars.next();
                }
            }
            Some(_) => {
                return Err(AwError::Mpp("expected ',' between parameters".into()));
            }
            None => break,
        }
    }

    Ok(params)
}

// --- Request Decoding ---

/// Decode the base64url request field into a PaymentRequest.
pub fn decode_request(request_b64: &str) -> Result<PaymentRequest, AwError> {
    let bytes = URL_SAFE_NO_PAD
        .decode(request_b64)
        .map_err(|e| AwError::Mpp(format!("invalid base64url in request field: {e}")))?;

    serde_json::from_slice(&bytes)
        .map_err(|e| AwError::Mpp(format!("invalid JSON in request field: {e}")))
}

// --- Budget Check ---

/// Convert the payment amount to human-readable units and check against budget.
/// Returns the human-readable amount if within budget.
pub fn check_budget(request: &PaymentRequest, max_cost: f64) -> Result<f64, AwError> {
    let amount_base: u64 = request
        .amount
        .parse()
        .map_err(|_| AwError::Mpp(format!("invalid amount: {}", request.amount)))?;
    let amount_human = amount_base as f64 / 10f64.powi(request.method_details.decimals as i32);

    if amount_human > max_cost {
        return Err(AwError::PriceExceeded {
            requested: amount_human,
            budget: max_cost,
            currency: request.currency.clone(),
        });
    }

    Ok(amount_human)
}

// --- Credential Building ---

/// Build the Authorization header value for the retry request.
pub fn build_authorization_header(challenge: &Challenge, tx_signature: &str) -> String {
    let credential = Credential {
        challenge: CredentialChallenge {
            id: challenge.id.clone(),
            realm: challenge.realm.clone(),
            method: challenge.method.clone(),
            intent: challenge.intent.clone(),
            request: challenge.request.clone(),
            expires: challenge.expires.clone(),
            digest: challenge.digest.clone(),
            opaque: challenge.opaque.clone(),
        },
        payload: Payload {
            r#type: "signature".to_string(),
            signature: tx_signature.to_string(),
        },
    };

    let json = serde_json::to_string(&credential).expect("credential serialization cannot fail");
    let encoded = URL_SAFE_NO_PAD.encode(json.as_bytes());
    format!("Payment {encoded}")
}

#[cfg(test)]
mod tests {
    use super::*;

    const EXAMPLE_REQUEST_B64: &str = "eyJhbW91bnQiOiIxMDAwIiwiY3VycmVuY3kiOiJFUGpGV2RkNUF1ZnFTU3FlTTJxTjF4enliYXBDOEc0d0VHR2tad3lURHQxdiIsIm1ldGhvZERldGFpbHMiOnsiZGVjaW1hbHMiOjYsIm5ldHdvcmsiOiJtYWlubmV0LWJldGEiLCJ0b2tlblByb2dyYW0iOiJUb2tlbmtlZ1FmZVp5aU53QUpiTmJHS1BGWENXdUJ2ZjlTczYyM1ZRNURBIn0sInJlY2lwaWVudCI6IjdLZWFBVlF6SDFFMWpKSEtRNlpKUXRqM0VhMnR3dnNLR0VHOGFNTnloYmFMIn0";

    fn example_header() -> String {
        format!(
            r#"Payment id="dG9rZW4tYWJj", realm="dns.sortis.dev", method="solana", intent="charge", request="{}", expires="2026-04-01T12:05:00Z""#,
            EXAMPLE_REQUEST_B64
        )
    }

    #[test]
    fn parse_known_good_header() {
        let challenge = parse_challenge(&example_header()).unwrap();
        assert_eq!(challenge.id, "dG9rZW4tYWJj");
        assert_eq!(challenge.realm, "dns.sortis.dev");
        assert_eq!(challenge.method, "solana");
        assert_eq!(challenge.intent, "charge");
        assert_eq!(challenge.request, EXAMPLE_REQUEST_B64);
        assert_eq!(challenge.expires.as_deref(), Some("2026-04-01T12:05:00Z"));
        assert!(challenge.description.is_none());
    }

    #[test]
    fn parse_minimal_header() {
        let header = format!(
            r#"Payment id="abc", realm="example.com", method="solana", intent="charge", request="dGVzdA""#
        );
        let challenge = parse_challenge(&header).unwrap();
        assert_eq!(challenge.id, "abc");
        assert!(challenge.expires.is_none());
        assert!(challenge.description.is_none());
        assert!(challenge.digest.is_none());
        assert!(challenge.opaque.is_none());
    }

    #[test]
    fn reject_missing_payment_scheme() {
        let result = parse_challenge("Bearer token=abc");
        assert!(result.is_err());
    }

    #[test]
    fn reject_missing_required_field() {
        let header = r#"Payment id="abc", realm="example.com", method="solana""#;
        let result = parse_challenge(header);
        assert!(result.is_err());
    }

    #[test]
    fn decode_request_from_example() {
        let req = decode_request(EXAMPLE_REQUEST_B64).unwrap();
        assert_eq!(req.amount, "1000");
        assert_eq!(req.currency, "EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v");
        assert_eq!(
            req.recipient,
            "7KeaAVQzH1E1jJHKQ6ZJQtj3Ea2twvsKGEG8aMNyhbaL"
        );
        assert_eq!(req.method_details.decimals, 6);
        assert_eq!(req.method_details.network, "mainnet-beta");
    }

    #[test]
    fn budget_check_within_budget() {
        let req = decode_request(EXAMPLE_REQUEST_B64).unwrap();
        let amount = check_budget(&req, 0.001).unwrap();
        assert!((amount - 0.001).abs() < f64::EPSILON);
    }

    #[test]
    fn budget_check_exceeds() {
        let req = decode_request(EXAMPLE_REQUEST_B64).unwrap();
        let result = check_budget(&req, 0.0009);
        assert!(matches!(result, Err(AwError::PriceExceeded { .. })));
    }

    #[test]
    fn credential_round_trip() {
        let challenge = parse_challenge(&example_header()).unwrap();
        let auth = build_authorization_header(&challenge, "5xKpFakeSignature");

        assert!(auth.starts_with("Payment "));

        // Decode and verify structure
        let encoded = auth.strip_prefix("Payment ").unwrap();
        let json_bytes = URL_SAFE_NO_PAD.decode(encoded).unwrap();
        let value: serde_json::Value = serde_json::from_slice(&json_bytes).unwrap();

        let chal = &value["challenge"];
        assert_eq!(chal["id"], "dG9rZW4tYWJj");
        assert_eq!(chal["realm"], "dns.sortis.dev");
        assert_eq!(chal["method"], "solana");
        assert_eq!(chal["intent"], "charge");
        // request must be the raw base64url string, not double-encoded
        assert_eq!(chal["request"], EXAMPLE_REQUEST_B64);
        assert_eq!(chal["expires"], "2026-04-01T12:05:00Z");
        // description must NOT be in the credential
        assert!(chal.get("description").is_none());

        let payload = &value["payload"];
        assert_eq!(payload["type"], "signature");
        assert_eq!(payload["signature"], "5xKpFakeSignature");
    }
}
