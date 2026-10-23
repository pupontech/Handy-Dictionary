//! All credentials stay in the native process. Endpoints cannot be supplied by the webview.
use crate::{decode, encode, Dictionary};
use base64::{engine::general_purpose::STANDARD, Engine};
use futures_util::StreamExt;
use reqwest::{Client, Response, StatusCode};
use serde::{de::DeserializeOwned, Deserialize, Serialize};
use std::time::{Duration, Instant};

pub const FILE_PATH: &str = "handy-dictionary.json";

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct Repository {
    pub owner: String,
    pub name: String,
    pub branch: String,
    pub id: u64,
}

pub struct Github {
    http: Client,
    api: String,
    login: String,
}

// Deliberately neither Debug nor Serialize: the device code never leaves Rust.
pub struct DeviceSession {
    pub user_code: String,
    device_code: String,
    expires: Instant,
    next_poll: Instant,
    pub interval: u64,
}

impl Github {
    pub fn new() -> Result<Self, String> {
        Ok(Self {
            http: Client::builder()
                .user_agent("Handy-Dictionary-Sync")
                .redirect(reqwest::redirect::Policy::none())
                .timeout(Duration::from_secs(30))
                .build()
                .map_err(|_| "network")?,
            api: "https://api.github.com".into(),
            login: "https://github.com".into(),
        })
    }
    pub async fn read(
        &self,
        token: &str,
        repo: &Repository,
    ) -> Result<Option<(Dictionary, String)>, String> {
        validate_name(&repo.owner)?;
        validate_name(&repo.name)?;
        let response = self
            .http
            .get(format!(
                "{}/repos/{}/{}/contents/{FILE_PATH}",
                self.api, repo.owner, repo.name
            ))
            .query(&[("ref", &repo.branch)])
            .bearer_auth(token)
            .send()
            .await
            .map_err(|_| "network")?;
        if response.status() == StatusCode::NOT_FOUND {
            return Ok(None);
        }
        #[derive(Deserialize)]
        struct File {
            content: String,
            encoding: String,
            sha: String,
            #[serde(rename = "type")]
            kind: String,
        }
        let file: File = json(response, 1_500_000).await?;
        if file.encoding != "base64" || file.kind != "file" || file.sha.is_empty() {
            return Err("invalid_response".into());
        }
        let bytes = STANDARD
            .decode(file.content.split_whitespace().collect::<String>())
            .map_err(|_| "invalid_dictionary")?;
        Ok(Some((decode(&bytes)?, file.sha)))
    }
    pub async fn write(
        &self,
        token: &str,
        repo: &Repository,
        dictionary: &Dictionary,
        sha: Option<&str>,
    ) -> Result<(), String> {
        validate_name(&repo.owner)?;
        validate_name(&repo.name)?;
        let mut payload = serde_json::json!({ "message": "Sync Handy Dictionary", "branch": repo.branch, "content": STANDARD.encode(encode(dictionary)?) });
        if let Some(sha) = sha {
            payload["sha"] = sha.into();
        }
        let response = self
            .http
            .put(format!(
                "{}/repos/{}/{}/contents/{FILE_PATH}",
                self.api, repo.owner, repo.name
            ))
            .bearer_auth(token)
            .json(&payload)
            .send()
            .await
            .map_err(|_| "network")?;
        let _: serde_json::Value = json(response, 64_000).await?;
        let verified = self.read(token, repo).await?.ok_or("remote_changed")?;
        if verified.0 != *dictionary {
            return Err("remote_changed".into());
        }
        Ok(())
    }
    pub async fn begin(&self, client_id: &str) -> Result<DeviceSession, String> {
        validate_client_id(client_id)?;
        #[derive(Deserialize)]
        struct Code {
            device_code: String,
            user_code: String,
            verification_uri: String,
            expires_in: u64,
            interval: u64,
        }
        let response = self
            .http
            .post(format!("{}/login/device/code", self.login))
            .header("Accept", "application/json")
            .form(&[("client_id", client_id)])
            .send()
            .await
            .map_err(|_| "network")?;
        let code: Code = json(response, 16_000).await?;
        if code.verification_uri != "https://github.com/login/device"
            || code.device_code.is_empty()
            || code.user_code.is_empty()
            || code.expires_in == 0
            || code.expires_in > 1800
        {
            return Err("invalid_response".into());
        }
        let interval = code.interval.clamp(5, 60);
        Ok(DeviceSession {
            user_code: code.user_code,
            device_code: code.device_code,
            expires: Instant::now() + Duration::from_secs(code.expires_in),
            next_poll: Instant::now() + Duration::from_secs(interval),
            interval,
        })
    }
    pub async fn poll(
        &self,
        client_id: &str,
        session: &mut DeviceSession,
    ) -> Result<Option<String>, String> {
        validate_client_id(client_id)?;
        if Instant::now() >= session.expires {
            return Err("authorization_expired".into());
        }
        if Instant::now() < session.next_poll {
            return Ok(None);
        }
        session.next_poll = Instant::now() + Duration::from_secs(session.interval);
        let response = self
            .http
            .post(format!("{}/login/oauth/access_token", self.login))
            .header("Accept", "application/json")
            .form(&[
                ("client_id", client_id),
                ("device_code", session.device_code.as_str()),
                ("grant_type", "urn:ietf:params:oauth:grant-type:device_code"),
            ])
            .send()
            .await
            .map_err(|_| "network")?;
        #[derive(Deserialize)]
        struct Token {
            access_token: Option<String>,
            token_type: Option<String>,
            error: Option<String>,
        }
        let token: Token = json(response, 16_000).await?;
        match token.error.as_deref() {
            Some("authorization_pending") => Ok(None),
            Some("slow_down") => {
                session.interval = session.interval.saturating_add(5);
                session.next_poll = Instant::now() + Duration::from_secs(session.interval);
                Ok(None)
            }
            Some("access_denied") => Err("authorization_denied".into()),
            Some("expired_token") => Err("authorization_expired".into()),
            Some(_) => Err("authorization_failed".into()),
            None => match (token.access_token, token.token_type.as_deref()) {
                (Some(token), Some("bearer")) if !token.is_empty() => Ok(Some(token)),
                _ => Err("invalid_response".into()),
            },
        }
    }
    pub async fn repository(
        &self,
        token: &str,
        owner: &str,
        name: &str,
    ) -> Result<Repository, String> {
        validate_name(owner)?;
        validate_name(name)?;
        #[derive(Deserialize)]
        struct Info {
            id: u64,
            default_branch: String,
            private: bool,
        }
        let response = self
            .http
            .get(format!("{}/repos/{owner}/{name}", self.api))
            .bearer_auth(token)
            .send()
            .await
            .map_err(|_| "network")?;
        let info: Info = json(response, 64_000).await?;
        if !info.private {
            return Err("private_repository_required".into());
        }
        Ok(Repository {
            owner: owner.into(),
            name: name.into(),
            branch: info.default_branch,
            id: info.id,
        })
    }
}

pub fn validate_client_id(value: &str) -> Result<(), String> {
    if !value.starts_with("Iv")
        || value.len() < 8
        || value.len() > 100
        || !value
            .bytes()
            .all(|c| c.is_ascii_alphanumeric() || c == b'.')
    {
        return Err("invalid_client_id".into());
    }
    Ok(())
}

pub fn validate_name(value: &str) -> Result<(), String> {
    if value.is_empty()
        || value.len() > 100
        || value == "."
        || value == ".."
        || !value
            .bytes()
            .all(|c| c.is_ascii_alphanumeric() || b"-_.".contains(&c))
    {
        return Err("invalid_repository".into());
    }
    Ok(())
}

async fn json<T: DeserializeOwned>(response: Response, limit: usize) -> Result<T, String> {
    let status = response.status();
    if !status.is_success() {
        return Err(match status {
            StatusCode::UNAUTHORIZED => "unauthorized",
            StatusCode::FORBIDDEN => "forbidden",
            StatusCode::NOT_FOUND => "not_found",
            StatusCode::CONFLICT | StatusCode::UNPROCESSABLE_ENTITY => "remote_changed",
            StatusCode::TOO_MANY_REQUESTS => "rate_limited",
            _ => "github_error",
        }
        .into());
    }
    let mut bytes = vec![];
    let mut stream = response.bytes_stream();
    while let Some(chunk) = stream.next().await {
        let chunk = chunk.map_err(|_| "network")?;
        if bytes.len() + chunk.len() > limit {
            return Err("response_too_large".into());
        }
        bytes.extend_from_slice(&chunk);
    }
    serde_json::from_slice(&bytes).map_err(|_| "invalid_response".into())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{
        io::{Read, Write},
        net::TcpListener,
    };
    fn script(responses: Vec<(u16, String)>) -> (Github, std::thread::JoinHandle<Vec<String>>) {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let address = format!("http://{}", listener.local_addr().unwrap());
        let handle = std::thread::spawn(move || {
            let mut requests = vec![];
            for (status, body) in responses {
                let (mut stream, _) = listener.accept().unwrap();
                stream
                    .set_read_timeout(Some(Duration::from_secs(5)))
                    .unwrap();
                let mut request = vec![];
                loop {
                    let mut buf = [0; 4096];
                    let n = stream.read(&mut buf).unwrap();
                    assert!(n > 0);
                    request.extend_from_slice(&buf[..n]);
                    if let Some(end) = request.windows(4).position(|w| w == b"\r\n\r\n") {
                        let headers = String::from_utf8_lossy(&request[..end]);
                        let length: usize = headers
                            .lines()
                            .find_map(|line| {
                                line.to_ascii_lowercase()
                                    .strip_prefix("content-length: ")
                                    .and_then(|v| v.parse().ok())
                            })
                            .unwrap_or(0);
                        if request.len() >= end + 4 + length {
                            break;
                        }
                    }
                }
                write!(
                    stream,
                    "HTTP/1.1 {status} Test\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                    body.len(),
                    body
                )
                .unwrap();
                requests.push(String::from_utf8(request).unwrap());
            }
            requests
        });
        let mut github = Github::new().unwrap();
        github.api = address.clone();
        github.login = address;
        (github, handle)
    }
    fn fixture(body: &str) -> (Github, std::thread::JoinHandle<Vec<String>>) {
        script(vec![(200, body.into())])
    }
    #[tokio::test]
    async fn reads_dictionary_directly_with_token_and_branch() {
        let dictionary = Dictionary {
            words: vec!["Handy".into()],
            replacements: vec![],
        };
        let body = serde_json::json!({"type":"file", "encoding":"base64", "sha":"revision-one", "content": STANDARD.encode(encode(&dictionary).unwrap())}).to_string();
        let (github, request) = fixture(&body);
        let repo = Repository {
            owner: "owner".into(),
            name: "dictionary".into(),
            branch: "my branch".into(),
            id: 1,
        };
        assert_eq!(
            github.read("fixture-token", &repo).await.unwrap(),
            Some((dictionary, "revision-one".into()))
        );
        let requests = request.join().unwrap();
        let request = &requests[0];
        assert!(request.starts_with(
            "GET /repos/owner/dictionary/contents/handy-dictionary.json?ref=my+branch "
        ));
        assert!(request.contains("authorization: Bearer fixture-token"));
    }
    #[tokio::test]
    async fn writes_with_compare_and_swap_then_reads_back() {
        let dictionary = Dictionary {
            words: vec!["Handy".into()],
            replacements: vec![],
        };
        let body = serde_json::json!({"type":"file", "encoding":"base64", "sha":"new", "content": STANDARD.encode(encode(&dictionary).unwrap())}).to_string();
        let (github, requests) = script(vec![(200, "{}".into()), (200, body)]);
        let repo = Repository {
            owner: "owner".into(),
            name: "dictionary".into(),
            branch: "main".into(),
            id: 1,
        };
        github
            .write("fixture-token", &repo, &dictionary, Some("old"))
            .await
            .unwrap();
        let requests = requests.join().unwrap();
        assert!(requests[0].starts_with("PUT "));
        let payload: serde_json::Value =
            serde_json::from_str(requests[0].split("\r\n\r\n").nth(1).unwrap()).unwrap();
        assert_eq!(payload["sha"], "old");
        assert_eq!(payload["branch"], "main");
        assert_eq!(
            decode(
                &STANDARD
                    .decode(payload["content"].as_str().unwrap())
                    .unwrap()
            )
            .unwrap(),
            dictionary
        );
        assert!(!payload.to_string().contains("fixture-token"));
        assert!(requests[1].starts_with("GET "));
    }
    #[tokio::test]
    async fn device_flow_keeps_device_secret_native_and_enforces_poll_delay() {
        let (github, requests) = script(vec![(200, r#"{"device_code":"native-secret","user_code":"ABCD-EFGH","verification_uri":"https://github.com/login/device","expires_in":900,"interval":5}"#.into()), (200, r#"{"access_token":"fixture-token","token_type":"bearer"}"#.into())]);
        let mut session = github.begin("Iv1.example").await.unwrap();
        assert_eq!(session.user_code, "ABCD-EFGH");
        assert_eq!(
            github.poll("Iv1.example", &mut session).await.unwrap(),
            None
        );
        session.next_poll = Instant::now();
        assert_eq!(
            github.poll("Iv1.example", &mut session).await.unwrap(),
            Some("fixture-token".into())
        );
        let requests = requests.join().unwrap();
        assert_eq!(requests.len(), 2);
        assert!(requests[0].contains("client_id=Iv1.example"));
        assert!(!requests[0].contains("scope="));
        assert!(requests[1].contains("device_code=native-secret"));
        assert!(!requests[1].contains("client_secret"));
    }
    #[tokio::test]
    async fn stale_write_is_rejected_without_readback_or_overwrite() {
        let (github, requests) = script(vec![(
            409,
            r#"{"message":"fixture-private-content"}"#.into(),
        )]);
        let repo = Repository {
            owner: "owner".into(),
            name: "dictionary".into(),
            branch: "main".into(),
            id: 1,
        };
        assert_eq!(
            github
                .write(
                    "fixture-token",
                    &repo,
                    &Dictionary::default(),
                    Some("stale")
                )
                .await,
            Err("remote_changed".into())
        );
        assert_eq!(requests.join().unwrap().len(), 1);
    }
    #[tokio::test]
    async fn successful_put_with_wrong_readback_is_failure() {
        let body = serde_json::json!({"type":"file", "encoding":"base64", "sha":"new", "content": STANDARD.encode(encode(&Dictionary::default()).unwrap())}).to_string();
        let (github, requests) = script(vec![(200, "{}".into()), (200, body)]);
        let repo = Repository {
            owner: "owner".into(),
            name: "dictionary".into(),
            branch: "main".into(),
            id: 1,
        };
        let desired = Dictionary {
            words: vec!["not-empty".into()],
            replacements: vec![],
        };
        assert_eq!(
            github
                .write("fixture-token", &repo, &desired, Some("old"))
                .await,
            Err("remote_changed".into())
        );
        assert_eq!(requests.join().unwrap().len(), 2);
    }
    #[tokio::test]
    async fn errors_are_sanitized_without_returning_github_body() {
        for (status, expected) in [
            (401, "unauthorized"),
            (403, "forbidden"),
            (404, "not_found"),
            (429, "rate_limited"),
            (500, "github_error"),
        ] {
            let (github, requests) = script(vec![(
                status,
                r#"{"message":"fixture-private-content"}"#.into(),
            )]);
            assert_eq!(
                github
                    .repository("fixture-token", "owner", "dictionary")
                    .await,
                Err(expected.into())
            );
            requests.join().unwrap();
        }
    }
    #[tokio::test]
    async fn response_size_is_bounded_before_parsing() {
        let (github, request) = fixture(&"x".repeat(65_000));
        assert_eq!(
            github
                .repository("fixture-token", "owner", "dictionary")
                .await,
            Err("response_too_large".into())
        );
        request.join().unwrap();
    }
    #[tokio::test]
    async fn redirects_are_not_followed_with_bearer_credentials() {
        let (github, requests) = script(vec![(302, "redirect-body".into())]);
        assert_eq!(
            github
                .repository("fixture-token", "owner", "dictionary")
                .await,
            Err("github_error".into())
        );
        assert_eq!(requests.join().unwrap().len(), 1);
    }
    #[tokio::test]
    async fn authorization_slowdown_increases_native_poll_delay() {
        let (github, requests) = fixture(r#"{"error":"slow_down"}"#);
        let mut session = DeviceSession {
            user_code: "ABCD-EFGH".into(),
            device_code: "native-secret".into(),
            expires: Instant::now() + Duration::from_secs(900),
            next_poll: Instant::now(),
            interval: 5,
        };
        assert_eq!(
            github.poll("Iv1.example", &mut session).await.unwrap(),
            None
        );
        assert_eq!(session.interval, 10);
        assert!(session.next_poll > Instant::now());
        assert_eq!(
            github.poll("Iv1.example", &mut session).await.unwrap(),
            None
        );
        assert_eq!(requests.join().unwrap().len(), 1);
    }
    #[tokio::test]
    async fn expired_session_does_not_make_network_request() {
        let github = Github::new().unwrap();
        let mut session = DeviceSession {
            user_code: "ABCD-EFGH".into(),
            device_code: "native-secret".into(),
            expires: Instant::now(),
            next_poll: Instant::now(),
            interval: 5,
        };
        assert_eq!(
            github.poll("Iv1.example", &mut session).await,
            Err("authorization_expired".into())
        );
    }
    #[tokio::test]
    async fn verification_uri_cannot_point_at_another_site() {
        let (github, requests) = fixture(
            r#"{"device_code":"native-secret","user_code":"ABCD-EFGH","verification_uri":"https://example.invalid/authorize","expires_in":900,"interval":5}"#,
        );
        assert!(matches!(github.begin("Iv1.example").await, Err(e) if e == "invalid_response"));
        requests.join().unwrap();
    }
    #[test]
    fn repository_path_injection_and_non_app_ids_are_rejected() {
        for input in [
            "",
            "..",
            "owner/repo",
            "https://github.com/owner",
            "repo?ref=other",
            "repo#fragment",
            "x%2Fy",
        ] {
            assert!(validate_name(input).is_err());
        }
        assert!(validate_client_id("Ov1.oauthapp").is_err());
        assert!(validate_client_id("Iv23validpublicid").is_ok());
    }
    #[tokio::test]
    async fn public_repository_is_refused() {
        let (github, request) = fixture(r#"{"id":1,"default_branch":"main","private":false}"#);
        assert_eq!(
            github
                .repository("fixture-token", "owner", "dictionary")
                .await,
            Err("private_repository_required".into())
        );
        let _ = request.join().unwrap();
    }
}
