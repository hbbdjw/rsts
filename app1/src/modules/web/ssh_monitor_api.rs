use actix_web::{web, HttpResponse, Responder};

#[test]
fn test_parse_cpu_usage() {
    let s1 = "cpu  100 0 100 800 0 0 0 0 0 0\n";
    let s2 = "cpu  110 0 140 820 0 0 0 0 0 0\n";
    let usage = parse_cpu_usage(s1, s2);
    assert!(usage <= 100);
    assert!(usage > 0);
}

use ssh2::{MethodType, Session as Ssh2Session};
use std::io::Read;
use std::net::TcpStream;
use std::sync::{Arc, Mutex};
use std::collections::HashMap;
use std::time::{Duration, Instant};
use once_cell::sync::Lazy;

// 全局缓存 SSH 会话，避免频繁握手
// Key: "hostname:port:username" (假设密码不变，或后续处理)
// Value: (Session, LastUsedTime)
struct SessionCache {
    sessions: HashMap<String, (Ssh2Session, Instant)>,
}

static SESSION_CACHE: Lazy<Arc<Mutex<SessionCache>>> = Lazy::new(|| {
    Arc::new(Mutex::new(SessionCache {
        sessions: HashMap::new(),
    }))
});

fn get_cached_session(host: &str, port: u16, username: &str, password: &str) -> anyhow::Result<Ssh2Session> {
    let key = format!("{}:{}:{}", host, port, username);
    let mut cache = SESSION_CACHE.lock().unwrap();
    
    // 清理过期会话 (> 5分钟未使用的)
    cache.sessions.retain(|_, (_, last_used)| last_used.elapsed() < Duration::from_secs(300));

    if let Some((sess, last_used)) = cache.sessions.get_mut(&key) {
        // 简单检查连接是否存活 (发送空 exec 或 ignore)
        // ssh2 没有直接的 ping，尝试 channel_session 创建来验证
        match sess.channel_session() {
            Ok(_) => {
                *last_used = Instant::now();
                return Ok(sess.clone());
            },
            Err(_) => {
                // 连接已失效，移除
                // 由于 get_mut 借用，这里不能直接 remove，只能标记或后续重新连接覆盖
            }
        }
    }
    
    // 建立新连接
    let sess = ssh2_connect(host, port, username, password)?;
    cache.sessions.insert(key, (sess.clone(), Instant::now()));
    Ok(sess)
}

use serde::{Deserialize, Serialize};

#[derive(Deserialize)]
pub struct MonitorRequest {
    pub hostname: String,
    pub port: Option<u16>,
    pub username: String,
    pub password: String,
}

#[derive(Serialize, Default)]
pub struct MemoryStat {
    pub used: u64,
    pub total: u64,
    pub usage: u8,
}

#[derive(Serialize, Default)]
pub struct NetworkStat {
    pub rx_rate: u64, // bytes per second
    pub tx_rate: u64, // bytes per second
}

#[derive(Serialize, Default)]
pub struct InterfaceStat {
    pub name: String,
    pub rx_rate: u64,
    pub tx_rate: u64,
}

#[derive(Serialize, Default)]
pub struct SystemStats {
    pub cpu: u8,
    pub memory: MemoryStat,
    pub swap: MemoryStat,
    pub network: NetworkStat, // Total
    pub interfaces: Vec<InterfaceStat>, // Per interface
}

fn parse_net_dev_detailed(output: &str) -> Vec<(String, u64, u64)> {
    let mut stats = Vec::new();
    for line in output.lines() {
        if line.contains(':') {
            let parts: Vec<&str> = line.split(':').collect();
            if parts.len() < 2 { continue; }
            let name = parts[0].trim().to_string();
            let val_str = parts[1].trim();
            let fields: Vec<&str> = val_str.split_whitespace().collect();
             if fields.len() >= 9 {
                 let rx = fields[0].parse::<u64>().unwrap_or(0);
                 let tx = fields[8].parse::<u64>().unwrap_or(0);
                 stats.push((name, rx, tx));
             }
        }
    }
    stats
}

fn calculate_net_rates(output1: &str, output2: &str) -> (NetworkStat, Vec<InterfaceStat>) {
    let stats1 = parse_net_dev_detailed(output1);
    let stats2 = parse_net_dev_detailed(output2);
    
    // Convert to map for easy lookup
    let map1: HashMap<String, (u64, u64)> = stats1.into_iter().map(|(n, r, t)| (n, (r, t))).collect();
    
    let mut interfaces = Vec::new();
    let mut total_rx = 0;
    let mut total_tx = 0;

    for (name, rx2, tx2) in stats2 {
        if let Some((rx1, tx1)) = map1.get(&name) {
            let rx_rate = rx2.saturating_sub(*rx1);
            let tx_rate = tx2.saturating_sub(*tx1);
            
            interfaces.push(InterfaceStat {
                name: name.clone(),
                rx_rate,
                tx_rate,
            });
            
            // Optionally exclude 'lo' from total if desired, but typically total includes all
            total_rx += rx_rate;
            total_tx += tx_rate;
        }
    }
    
    (
        NetworkStat { rx_rate: total_rx, tx_rate: total_tx },
        interfaces
    )
}

fn parse_net_dev_simple(output: &str) -> (u64, u64) {
    let mut total_rx = 0;
    let mut total_tx = 0;
    
    for line in output.lines() {
        if !line.contains(':') { continue; }
        let parts: Vec<&str> = line.split(':').collect();
        if parts.len() < 2 { continue; }
        
        let stats_part = parts[1].trim();
        let fields: Vec<&str> = stats_part.split_whitespace().collect();
        
        // Field 0: RX bytes, Field 8: TX bytes
        if fields.len() >= 9 {
            // Ignore lo interface? usually user wants total traffic including loopback or not?
            // Let's include everything for now, or exclude lo if needed.
            // if parts[0].trim() == "lo" { continue; }
            
            if let Some(rx) = fields[0].parse::<u64>().ok() {
                total_rx += rx;
            }
            if let Some(tx) = fields[8].parse::<u64>().ok() {
                total_tx += tx;
            }
        }
    }
    (total_rx, total_tx)
}

fn ssh2_connect(host: &str, port: u16, username: &str, password: &str) -> anyhow::Result<Ssh2Session> {
    let addr = format!("{}:{}", host, port);
    let tcp = TcpStream::connect(addr)?;
    tcp.set_read_timeout(Some(Duration::from_secs(30)))?;
    tcp.set_write_timeout(Some(Duration::from_secs(30)))?;
    let mut sess = Ssh2Session::new()?;
    sess.set_tcp_stream(tcp);
    // 扩大兼容算法集合
    let _ = sess.method_pref(
        MethodType::Kex,
        "curve25519-sha256,curve25519-sha256@libssh.org,ecdh-sha2-nistp256,ecdh-sha2-nistp384,ecdh-sha2-nistp521,diffie-hellman-group-exchange-sha256,diffie-hellman-group16-sha512,diffie-hellman-group18-sha512,diffie-hellman-group14-sha256",
    );
    let _ = sess.method_pref(
        MethodType::HostKey,
        "ecdsa-sha2-nistp256,ecdsa-sha2-nistp384,ecdsa-sha2-nistp521,ssh-ed25519,rsa-sha2-512,rsa-sha2-256,ssh-rsa",
    );
    let crypt = "aes128-ctr,aes192-ctr,aes256-ctr,aes128-gcm@openssh.com,aes256-gcm@openssh.com,chacha20-poly1305@openssh.com";
    let _ = sess.method_pref(MethodType::CryptCs, crypt);
    let _ = sess.method_pref(MethodType::CryptSc, crypt);
    let mac = "hmac-sha2-256,hmac-sha2-512,hmac-sha2-256-etm@openssh.com,hmac-sha2-512-etm@openssh.com";
    let _ = sess.method_pref(MethodType::MacCs, mac);
    let _ = sess.method_pref(MethodType::MacSc, mac);
    sess.handshake()?;
    sess.set_blocking(true);
    sess.userauth_password(username, password)?;
    if !sess.authenticated() {
        anyhow::bail!("ssh auth failed");
    }
    Ok(sess)
}

fn ssh2_exec(sess: &Ssh2Session, cmd: &str) -> anyhow::Result<String> {
    let mut ch = sess.channel_session()?;
    ch.exec(cmd)?;
    let mut out = String::new();
    ch.read_to_string(&mut out)?;
    ch.wait_close()?;
    Ok(out)
}

fn parse_meminfo(meminfo: &str) -> (MemoryStat, MemoryStat) {
    let mut mem_total = 0u64;
    let mut mem_free = 0u64;
    let mut buffers = 0u64;
    let mut cached = 0u64;
    let mut sreclaimable = 0u64;
    let mut shmem = 0u64;
    let mut swap_total = 0u64;
    let mut swap_free = 0u64;

    for line in meminfo.lines() {
        let mut parts = line.split_whitespace();
        if let Some(key) = parts.next() {
            let val = parts.next().and_then(|v| v.parse::<u64>().ok()).unwrap_or(0);
            match key {
                "MemTotal:" => mem_total = val * 1024,
                "MemFree:" => mem_free = val * 1024,
                "Buffers:" => buffers = val * 1024,
                "Cached:" => cached = val * 1024,
                "SReclaimable:" => sreclaimable = val * 1024,
                "Shmem:" => shmem = val * 1024,
                "SwapTotal:" => swap_total = val * 1024,
                "SwapFree:" => swap_free = val * 1024,
                _ => {}
            }
        }
    }
    let mem_available = mem_free + buffers + cached + sreclaimable - shmem;
    let mem_used = mem_total.saturating_sub(mem_available);
    let mem_usage = if mem_total > 0 {
        ((mem_used as f64 / mem_total as f64) * 100.0).round() as u8
    } else {
        0
    };

    let swap_used = swap_total.saturating_sub(swap_free);
    let swap_usage = if swap_total > 0 {
        ((swap_used as f64 / swap_total as f64) * 100.0).round() as u8
    } else {
        0
    };

    (
        MemoryStat {
            used: mem_used,
            total: mem_total,
            usage: mem_usage,
        },
        MemoryStat {
            used: swap_used,
            total: swap_total,
            usage: swap_usage,
        },
    )
}

fn parse_cpu_usage(stat1: &str, stat2: &str) -> u8 {
    fn parse_line(s: &str) -> Option<(u64, u64)> {
        let line = s.lines().next()?.trim();
        if !line.starts_with("cpu ") {
            return None;
        }
        let nums: Vec<u64> = line
            .split_whitespace()
            .skip(1)
            .filter_map(|v| v.parse::<u64>().ok())
            .collect();
        if nums.len() < 8 {
            return None;
        }
        let user = nums[0];
        let nice = nums[1];
        let system = nums[2];
        let idle = nums[3];
        let iowait = nums.get(4).copied().unwrap_or(0);
        let irq = nums.get(5).copied().unwrap_or(0);
        let softirq = nums.get(6).copied().unwrap_or(0);
        let steal = nums.get(7).copied().unwrap_or(0);
        let total = user + nice + system + idle + iowait + irq + softirq + steal;
        Some((idle + iowait, total))
    }
    let (idle1, total1) = match parse_line(stat1) {
        Some(v) => v,
        None => return 0,
    };
    let (idle2, total2) = match parse_line(stat2) {
        Some(v) => v,
        None => return 0,
    };
    let idle_d = idle2.saturating_sub(idle1);
    let total_d = total2.saturating_sub(total1);
    if total_d == 0 {
        return 0;
    }
    let usage = 100.0 * (1.0 - idle_d as f64 / total_d as f64);
    usage.round().clamp(0.0, 100.0) as u8
}

pub async fn get_monitor_stats(payload: web::Json<MonitorRequest>) -> impl Responder {
    let host = payload.hostname.trim();
    let port = payload.port.unwrap_or(22);
    let user = payload.username.trim();
    let pass = payload.password.as_str(); // 不记录日志

    let sess = match get_cached_session(host, port, user, pass) {
        Ok(s) => s,
        Err(e) => {
            // 返回 200 OK 但带有错误信息的 JSON，而不是 400 Bad Request
            // 这样前端 JSON 解析不会报错，或者我们应该在前端处理 400
            // 但为了简单起见，这里记录错误并返回空数据结构或特定错误字段
            // 不过前端是 expect 200 OK。
            // 让我们保持 400，但前端需要捕获。
            // 这里的 400 是因为连接失败。
            return HttpResponse::BadRequest().json(serde_json::json!({
                "error": format!("ssh connect failed: {}", e),
            }));
        }
    };

    // CPU: 两次采样
    // 增加 /proc/net/dev 读取
    let cmd1 = "cat /proc/stat; echo '---'; cat /proc/net/dev";
    let output1 = match ssh2_exec(&sess, cmd1) {
        Ok(s) => s,
        Err(_) => String::new(),
    };
    
    // 缩短等待时间以避免超时，前端可以平滑处理
    // 或许不需要 sleep，但为了计算速率需要时间差。
    // 注意：如果 actix-web 默认超时较短（如 5秒），1秒 sleep 是安全的。
    // 但是前端轮询间隔 2秒，接口耗时 >1秒 可能导致堆积。
    // 让我们保持 1秒，但确保不阻塞太久。
    std::thread::sleep(Duration::from_millis(1000));
    
    let cmd2 = "cat /proc/stat; echo '---'; cat /proc/net/dev";
    let output2 = match ssh2_exec(&sess, cmd2) {
        Ok(s) => s,
        Err(_) => String::new(),
    };

    // 解析 CPU 和 Network
    let (cpu, network, interfaces) = if !output1.is_empty() && !output2.is_empty() {
        let parts1: Vec<&str> = output1.split("---").collect();
        let parts2: Vec<&str> = output2.split("---").collect();
        
        let cpu_val = if parts1.len() > 0 && parts2.len() > 0 {
            parse_cpu_usage(parts1[0], parts2[0])
        } else {
            0
        };
        
        let (net_stat, iface_stats) = if parts1.len() > 1 && parts2.len() > 1 {
            calculate_net_rates(parts1[1], parts2[1])
        } else {
            (NetworkStat::default(), Vec::new())
        };
        
        (cpu_val, net_stat, iface_stats)
    } else {
        (0, NetworkStat::default(), Vec::new())
    };

    // 内存/Swap
    let meminfo = match ssh2_exec(&sess, "cat /proc/meminfo") {
        Ok(s) => s,
        Err(_) => String::new(),
    };
    let (memory, swap) = if !meminfo.is_empty() {
        parse_meminfo(&meminfo)
    } else {
        (MemoryStat::default(), MemoryStat::default())
    };

    let result = SystemStats {
        cpu,
        memory,
        swap,
        network,
        interfaces,
    };
    HttpResponse::Ok().json(result)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_meminfo() {
        let sample = r#"MemTotal:       16377824 kB
MemFree:         123456 kB
Buffers:          23456 kB
Cached:           345678 kB
SReclaimable:      45678 kB
Shmem:             5678 kB
SwapTotal:       2097152 kB
SwapFree:        1048576 kB
"#;
        let (mem, swap) = parse_meminfo(sample);
        assert!(mem.total > 0);
        assert!(swap.total == 2097152 * 1024);
        assert!(swap.used == (2097152 - 1048576) * 1024);
    }

    #[test]
    fn test_calculate_net_rates() {
        let output1 = r#"Inter-|   Receive                                                |  Transmit
 face |bytes    packets errs drop fifo frame compressed multicast|bytes    packets errs drop fifo colls carrier compressed
    lo: 48937000   44365    0    0    0     0          0         0 48937000   44365    0    0    0     0       0          0
  eth0: 1048576   12345    0    0    0     0          0         0  2097152    6789    0    0    0     0       0          0
"#;
        let output2 = r#"Inter-|   Receive                                                |  Transmit
 face |bytes    packets errs drop fifo frame compressed multicast|bytes    packets errs drop fifo colls carrier compressed
    lo: 48938000   44365    0    0    0     0          0         0 48938000   44365    0    0    0     0       0          0
  eth0: 1049576   12345    0    0    0     0          0         0  2098152    6789    0    0    0     0       0          0
"#;
        let (total, ifaces) = calculate_net_rates(output1, output2);
        
        // lo: +1000, +1000
        // eth0: +1000, +1000
        // total: +2000, +2000
        assert_eq!(total.rx_rate, 2000);
        assert_eq!(total.tx_rate, 2000);
        
        let eth0 = ifaces.iter().find(|i| i.name == "eth0").unwrap();
        assert_eq!(eth0.rx_rate, 1000);
        assert_eq!(eth0.tx_rate, 1000);
    }
}

