use std::io::{self, Read, Write};
use std::net::{TcpStream, ToSocketAddrs};
use std::time::Duration;

/// 发起 TCP 请求服务端
#[derive(Debug, Clone)]
pub struct DialTcpRequest {
    /// 服务器地址
    pub addr: String,
    /// 服务器端口
    pub port: u16,
    /// 连接超时时间（秒），None 表示使用默认值
    pub timeout: Option<u64>,
    /// 读取超时时间（秒），None 表示无超时
    pub read_timeout: Option<u64>,
    /// 写入超时时间（秒），None 表示无超时
    pub write_timeout: Option<u64>,
    /// 是否启用 TCP_NODELAY (禁用 Nagle 算法)
    pub nodelay: bool,
}

impl DialTcpRequest {
    /// 创建一个新的 TCP 请求配置
    pub fn new(addr: impl Into<String>, port: u16) -> Self {
        Self {
            addr: addr.into(),
            port,
            timeout: Some(30),
            read_timeout: Some(30),
            write_timeout: Some(30),
            nodelay: false,
        }
    }

    /// 设置连接超时时间（秒）
    pub fn with_timeout(mut self, timeout: u64) -> Self {
        self.timeout = Some(timeout);
        self
    }

    /// 设置读取超时时间（秒）
    pub fn with_read_timeout(mut self, timeout: u64) -> Self {
        self.read_timeout = Some(timeout);
        self
    }

    /// 设置写入超时时间（秒）
    pub fn with_write_timeout(mut self, timeout: u64) -> Self {
        self.write_timeout = Some(timeout);
        self
    }

    /// 设置是否启用 TCP_NODELAY
    pub fn with_nodelay(mut self, nodelay: bool) -> Self {
        self.nodelay = nodelay;
        self
    }

    /// 获取完整的服务器地址
    pub fn socket_addr(&self) -> String {
        format!("{}:{}", self.addr, self.port)
    }

    /// 建立 TCP 连接
    pub fn dial(&self) -> io::Result<TcpStream> {
        let addr = self.socket_addr();

        // 建立连接
        let stream = if let Some(timeout) = self.timeout {
            // 使用超时连接
            let addrs: Vec<_> = addr.to_socket_addrs()?.collect();
            if addrs.is_empty() {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidInput,
                    "could not resolve to any addresses",
                ));
            }
            TcpStream::connect_timeout(&addrs[0], Duration::from_secs(timeout))?
        } else {
            // 不使用超时
            TcpStream::connect(&addr)?
        };

        // 设置读取超时
        if let Some(timeout) = self.read_timeout {
            stream.set_read_timeout(Some(Duration::from_secs(timeout)))?;
        }

        // 设置写入超时
        if let Some(timeout) = self.write_timeout {
            stream.set_write_timeout(Some(Duration::from_secs(timeout)))?;
        }

        // 设置 TCP_NODELAY
        stream.set_nodelay(self.nodelay)?;

        Ok(stream)
    }

    /// 发送数据并接收响应
    pub fn send_and_receive(&self, data: &[u8]) -> io::Result<Vec<u8>> {
        let mut stream = self.dial()?;

        // 发送数据
        stream.write_all(data)?;
        stream.flush()?;

        // 接收数据
        let mut buffer = Vec::new();
        stream.read_to_end(&mut buffer)?;

        Ok(buffer)
    }

    /// 发送数据并接收固定长度的响应
    pub fn send_and_receive_fixed(&self, data: &[u8], response_len: usize) -> io::Result<Vec<u8>> {
        let mut stream = self.dial()?;

        // 发送数据
        stream.write_all(data)?;
        stream.flush()?;

        // 接收固定长度数据
        let mut buffer = vec![0u8; response_len];
        stream.read_exact(&mut buffer)?;

        Ok(buffer)
    }
}

impl Default for DialTcpRequest {
    fn default() -> Self {
        Self::new("127.0.0.1", 8080)
    }
}
