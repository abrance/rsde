/// HttpRequest 使用示例和测试

#[cfg(test)]
mod tests {
    use super::super::{HttpBody, HttpMethod, HttpRequest};
    use std::collections::HashMap;

    #[test]
    fn test_new_request() {
        let request = HttpRequest::new("https://example.com");
        assert_eq!(request.url, "https://example.com");
        assert_eq!(request.method, HttpMethod::GET);
        assert_eq!(request.timeout, Some(30));
        assert_eq!(request.follow_redirects, true);
    }

    #[test]
    fn test_get_request() {
        let request = HttpRequest::get("https://api.example.com/users");
        assert_eq!(request.method, HttpMethod::GET);
        assert_eq!(request.url, "https://api.example.com/users");
    }

    #[test]
    fn test_post_request() {
        let request = HttpRequest::post("https://api.example.com/users");
        assert_eq!(request.method, HttpMethod::POST);
    }

    #[test]
    fn test_with_headers() {
        let request = HttpRequest::get("https://example.com")
            .with_header("Authorization", "Bearer token123")
            .with_header("User-Agent", "MyApp/1.0");

        assert_eq!(
            request.headers.get("Authorization"),
            Some(&"Bearer token123".to_string())
        );
        assert_eq!(
            request.headers.get("User-Agent"),
            Some(&"MyApp/1.0".to_string())
        );
    }

    #[test]
    fn test_with_timeout() {
        let request = HttpRequest::get("https://example.com").with_timeout(10);
        assert_eq!(request.timeout, Some(10));
    }

    #[test]
    fn test_with_text_body() {
        let request = HttpRequest::post("https://example.com").with_text("Hello, World!");

        match request.body {
            HttpBody::Text(ref text) => assert_eq!(text, "Hello, World!"),
            _ => panic!("Expected Text body"),
        }
    }

    #[test]
    fn test_with_form_body() {
        let mut form = HashMap::new();
        form.insert("username".to_string(), "user123".to_string());
        form.insert("password".to_string(), "pass456".to_string());

        let request = HttpRequest::post("https://example.com/login").with_form(form.clone());

        match request.body {
            HttpBody::Form(ref f) => {
                assert_eq!(f.get("username"), Some(&"user123".to_string()));
                assert_eq!(f.get("password"), Some(&"pass456".to_string()));
            }
            _ => panic!("Expected Form body"),
        }

        assert_eq!(
            request.headers.get("Content-Type"),
            Some(&"application/x-www-form-urlencoded".to_string())
        );
    }

    #[test]
    fn test_with_json_body() {
        use serde_json::json;

        let json_data = json!({
            "name": "John Doe",
            "age": 30
        });

        let request = HttpRequest::post("https://api.example.com/users")
            .with_json(&json_data)
            .unwrap();

        match request.body {
            HttpBody::Json(ref j) => {
                assert_eq!(j["name"], "John Doe");
                assert_eq!(j["age"], 30);
            }
            _ => panic!("Expected JSON body"),
        }

        assert_eq!(
            request.headers.get("Content-Type"),
            Some(&"application/json".to_string())
        );
    }

    #[test]
    fn test_builder_pattern() {
        let request = HttpRequest::post("https://api.example.com/data")
            .with_header("Authorization", "Bearer token")
            .with_timeout(15)
            .with_follow_redirects(false)
            .with_max_redirects(5)
            .with_verify_ssl(true)
            .with_text("test data");

        assert_eq!(request.method, HttpMethod::POST);
        assert_eq!(request.timeout, Some(15));
        assert_eq!(request.follow_redirects, false);
        assert_eq!(request.max_redirects, 5);
        assert_eq!(request.verify_ssl, true);
    }

    #[test]
    fn test_method_helpers() {
        assert_eq!(HttpMethod::GET.as_str(), "GET");
        assert_eq!(HttpMethod::POST.as_str(), "POST");
        assert_eq!(HttpMethod::PUT.as_str(), "PUT");
        assert_eq!(HttpMethod::DELETE.as_str(), "DELETE");
        assert_eq!(HttpMethod::PATCH.as_str(), "PATCH");
        assert_eq!(HttpMethod::HEAD.as_str(), "HEAD");
        assert_eq!(HttpMethod::OPTIONS.as_str(), "OPTIONS");
    }

    #[test]
    fn test_default() {
        let request = HttpRequest::default();
        assert_eq!(request.url, "http://localhost");
        assert_eq!(request.method, HttpMethod::GET);
    }

    // 实际的网络请求测试（需要网络连接）
    #[test]
    #[ignore]
    fn test_send_get_request() {
        let request = HttpRequest::get("https://httpbin.org/get")
            .with_header("User-Agent", "HttpRequest/1.0");

        match request.send_text() {
            Ok(text) => {
                println!("Response: {}", text);
                assert!(text.contains("httpbin"));
            }
            Err(e) => {
                eprintln!("Request failed: {}", e);
            }
        }
    }

    #[test]
    #[ignore]
    fn test_send_post_json() {
        use serde_json::json;

        let json_data = json!({
            "name": "Test User",
            "email": "test@example.com"
        });

        let request = HttpRequest::post("https://httpbin.org/post")
            .with_json(&json_data)
            .unwrap();

        match request.send_text() {
            Ok(text) => {
                println!("Response: {}", text);
                assert!(text.contains("Test User"));
            }
            Err(e) => {
                eprintln!("Request failed: {}", e);
            }
        }
    }

    #[test]
    #[ignore]
    fn test_send_with_timeout() {
        let request = HttpRequest::get("https://httpbin.org/delay/2").with_timeout(1);

        match request.send() {
            Ok(_) => println!("Request succeeded"),
            Err(e) => {
                println!("Request failed (expected timeout): {}", e);
                assert!(e.is_timeout());
            }
        }
    }
}
