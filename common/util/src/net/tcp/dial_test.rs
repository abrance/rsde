/// DialTcpRequest 使用示例

#[cfg(test)]
mod tests {
    use super::super::DialTcpRequest;

    #[test]
    #[ignore] // 需要实际的服务器才能运行
    fn test_basic_dial() {
        let request = DialTcpRequest::new("127.0.0.1", 8080);

        match request.dial() {
            Ok(_stream) => {
                println!("连接成功");
            }
            Err(e) => {
                println!("连接失败: {}", e);
            }
        }
    }

    #[test]
    #[ignore]
    fn test_dial_with_custom_timeout() {
        let request = DialTcpRequest::new("127.0.0.1", 8080)
            .with_timeout(5)
            .with_read_timeout(10)
            .with_write_timeout(10)
            .with_nodelay(true);

        match request.dial() {
            Ok(_stream) => {
                println!("连接成功");
            }
            Err(e) => {
                println!("连接失败: {}", e);
            }
        }
    }

    #[test]
    #[ignore]
    fn test_send_and_receive() {
        let request = DialTcpRequest::new("127.0.0.1", 8080)
            .with_timeout(5)
            .with_nodelay(true);

        let data = b"Hello, Server!";

        match request.send_and_receive(data) {
            Ok(response) => {
                println!("收到响应: {:?}", String::from_utf8_lossy(&response));
            }
            Err(e) => {
                println!("请求失败: {}", e);
            }
        }
    }

    #[test]
    fn test_socket_addr() {
        let request = DialTcpRequest::new("example.com", 443);
        assert_eq!(request.socket_addr(), "example.com:443");
    }

    #[test]
    fn test_default() {
        let request = DialTcpRequest::default();
        assert_eq!(request.addr, "127.0.0.1");
        assert_eq!(request.port, 8080);
    }

    #[test]
    fn test_builder_pattern() {
        let request = DialTcpRequest::new("example.com", 443)
            .with_timeout(10)
            .with_read_timeout(15)
            .with_write_timeout(15)
            .with_nodelay(true);

        assert_eq!(request.addr, "example.com");
        assert_eq!(request.port, 443);
        assert_eq!(request.timeout, Some(10));
        assert_eq!(request.read_timeout, Some(15));
        assert_eq!(request.write_timeout, Some(15));
        assert_eq!(request.nodelay, true);
    }
}
