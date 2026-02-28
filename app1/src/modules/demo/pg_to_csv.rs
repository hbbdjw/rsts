use anyhow::{Context, Result};
use bytes::Bytes;
use serde::Deserialize;
use std::io::Write;
use std::path::Path;
use std::sync::Arc;
use std::sync::atomic::AtomicUsize;
use std::time::Duration;
use tokio_postgres::NoTls;
/// 异步将PostgreSQL数据库表中的数据转换为CSV文件
///
/// # Arguments
///
/// * `tableName` - 数据库表名
/// * `filename` - 要写入的CSV文件名
///
/// # Returns
///
/// * `Ok(ret)` - 成功将数据写入CSV文件
/// * `Err(e)` - 写入CSV文件失败，返回错误信息
pub struct CsvExportOptions {
    pub gzip: bool,
    pub buf_size_bytes: usize,
    pub partitions: usize,
    pub rows_per_query: Option<usize>,
}

impl Default for CsvExportOptions {
    fn default() -> Self {
        Self {
            gzip: false,
            buf_size_bytes: 32 << 20,
            partitions: 1,
            rows_per_query: None,
        }
    }
}

fn build_timestamped_filename(filename: &str, gzip: bool) -> String {
    let ts = chrono::Local::now().format("%Y%m%d%H%M%S").to_string();
    let path = Path::new(filename);
    let parent = path.parent().map(|p| p.to_path_buf());
    let stem = path
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or(filename);
    let ext = path.extension().and_then(|s| s.to_str());
    let new_name = if gzip {
        format!("{}_{}.csv.gz", stem, ts)
    } else if let Some(ext) = ext {
        format!("{}_{}.{}", stem, ts, ext)
    } else {
        format!("{}_{}", stem, ts)
    };
    let final_path = if let Some(parent) = parent {
        parent.join(new_name)
    } else {
        std::path::PathBuf::from(new_name)
    };
    final_path.to_string_lossy().to_string()
}

async fn copy_to_file<W: std::io::Write + Send>(
    client: &tokio_postgres::Client,
    sql: String,
    writer: &mut W,
    _total_rows: &Arc<AtomicUsize>,
    buf_tick: Duration,
    out_name: &str,
) -> Result<()> {
    use futures::StreamExt;
    let stream = client.copy_out(&sql).await.context("copy_out failed")?;
    futures::pin_mut!(stream);
    let mut last = std::time::Instant::now();
    let mut bytes_written: usize = 0;
    while let Some(chunk_res) = stream.next().await {
        let chunk: Bytes = chunk_res.context("stream chunk error")?;
        writer
            .write_all(&chunk)
            .with_context(|| format!("write chunk failed: {}", out_name))?;
        bytes_written += chunk.len();
        if last.elapsed() >= buf_tick {
            let mb = (bytes_written as f64) / (1024.0 * 1024.0);
            println!("CSV写入进度: 已写入 {:.2} MB", mb);
            last = std::time::Instant::now();
        }
    }
    Ok(())
}

pub async fn pg_transfor_to_csv_async_with_options_dsn(
    table_full: &str,
    filename: &str,
    opts: CsvExportOptions,
    dsn: &str,
) -> Result<()> {
    let final_filename = build_timestamped_filename(filename, opts.gzip);
    let (client, connection) =
        tokio::time::timeout(Duration::from_secs(10), tokio_postgres::connect(dsn, NoTls))
            .await
            .context("connect timeout")?
            .context("connect failed")?;
    tokio::spawn(async move {
        let _ = connection.await;
    });
    let total_rows = Arc::new(AtomicUsize::new(0));
    if opts.partitions <= 1 {
        use std::fs::OpenOptions;
        use std::io::BufWriter;
        if opts.gzip {
            use flate2::Compression;
            use flate2::write::GzEncoder;
            if let Some(dir) = Path::new(&final_filename).parent() {
                std::fs::create_dir_all(dir).ok();
            }
            let file = OpenOptions::new()
                .create(true)
                .write(true)
                .truncate(true)
                .open(&final_filename)
                .context("open output file failed")?;
            let mut writer = GzEncoder::new(
                BufWriter::with_capacity(opts.buf_size_bytes, file),
                Compression::fast(),
            );
            if let Some(rows) = opts.rows_per_query {
                let count_row = client
                    .query_one(&format!("select count(*) from {}", table_full), &[])
                    .await
                    .context("count query failed")?;
                let total: i64 = count_row.get(0);
                let mut offset: i64 = 0;
                while offset < total {
                    let limit = std::cmp::min(rows as i64, total - offset);
                    let sql = format!(
                        "COPY (SELECT * FROM {} LIMIT {} OFFSET {}) TO STDOUT WITH (FORMAT csv, DELIMITER ',', QUOTE '\"', ESCAPE '\"')",
                        table_full, limit, offset
                    );
                    copy_to_file(
                        &client,
                        sql,
                        &mut writer,
                        &total_rows,
                        Duration::from_secs(5),
                        &final_filename,
                    )
                    .await?;
                    offset += limit;
                }
            } else {
                let sql = format!(
                    "COPY (SELECT * FROM {}) TO STDOUT WITH (FORMAT csv, DELIMITER ',', QUOTE '\"', ESCAPE '\"')",
                    table_full
                );
                copy_to_file(
                    &client,
                    sql,
                    &mut writer,
                    &total_rows,
                    Duration::from_secs(5),
                    &final_filename,
                )
                .await?;
            }
            writer.flush().context("flush gzip writer failed")?;
        } else {
            if let Some(dir) = Path::new(&final_filename).parent() {
                std::fs::create_dir_all(dir).ok();
            }
            let file = OpenOptions::new()
                .create(true)
                .write(true)
                .truncate(true)
                .open(&final_filename)
                .context("open output file failed")?;
            let mut writer = BufWriter::with_capacity(opts.buf_size_bytes, file);
            if let Some(rows) = opts.rows_per_query {
                let count_row = client
                    .query_one(&format!("select count(*) from {}", table_full), &[])
                    .await
                    .context("count query failed")?;
                let total: i64 = count_row.get(0);
                let mut offset: i64 = 0;
                while offset < total {
                    let limit = std::cmp::min(rows as i64, total - offset);
                    let sql = format!(
                        "COPY (SELECT * FROM {} LIMIT {} OFFSET {}) TO STDOUT WITH (FORMAT csv, DELIMITER ',', QUOTE '\"', ESCAPE '\"')",
                        table_full, limit, offset
                    );
                    copy_to_file(
                        &client,
                        sql,
                        &mut writer,
                        &total_rows,
                        Duration::from_secs(5),
                        &final_filename,
                    )
                    .await?;
                    offset += limit;
                }
            } else {
                let sql = format!(
                    "COPY (SELECT * FROM {}) TO STDOUT WITH (FORMAT csv, DELIMITER ',', QUOTE '\"', ESCAPE '\"')",
                    table_full
                );
                copy_to_file(
                    &client,
                    sql,
                    &mut writer,
                    &total_rows,
                    Duration::from_secs(5),
                    &final_filename,
                )
                .await?;
            }
            writer.flush().context("flush writer failed")?;
        }
    } else {
        let count_row = client
            .query_one(&format!("select count(*) from {}", table_full), &[])
            .await
            .context("count query failed")?;
        let total: i64 = count_row.get(0);
        let parts = opts.partitions as i64;
        let per = ((total + parts - 1) / parts) as i64;
        let mut handles = Vec::new();
        for i in 0..parts {
            let offset = i * per;
            let limit = std::cmp::min(per, total - offset);
            if limit <= 0 {
                continue;
            }
            let dsn_owned = dsn.to_string();
            let table_owned = table_full.to_string();
            let total_rows_clone = total_rows.clone();
            let part_name = {
                let p = Path::new(&final_filename);
                let parent = p.parent().map(|p| p.to_path_buf());
                let stem = p
                    .file_stem()
                    .and_then(|s| s.to_str())
                    .unwrap_or(&final_filename);
                let ext = if opts.gzip {
                    "csv"
                } else {
                    p.extension().and_then(|s| s.to_str()).unwrap_or("csv")
                };
                let name = format!("{}.part{}.{}", stem, i, ext);
                let pp = if let Some(parent) = parent {
                    parent.join(name)
                } else {
                    std::path::PathBuf::from(name)
                };
                pp.to_string_lossy().to_string()
            };
            let buf_size = opts.buf_size_bytes;
            let handle = tokio::spawn(async move {
                use std::fs::OpenOptions;
                use std::io::BufWriter;
                let sql = format!(
                    "COPY (SELECT * FROM {} LIMIT {} OFFSET {}) TO STDOUT WITH (FORMAT csv, DELIMITER ',', QUOTE '\"', ESCAPE '\"')",
                    table_owned, limit, offset
                );
                let (client_local, conn_local) = match tokio::time::timeout(
                    Duration::from_secs(10),
                    tokio_postgres::connect(&dsn_owned, NoTls),
                )
                .await
                {
                    Ok(Ok(v)) => v,
                    _ => return,
                };
                tokio::spawn(async move {
                    let _ = conn_local.await;
                });
                let file = match OpenOptions::new()
                    .create(true)
                    .write(true)
                    .truncate(true)
                    .open(&part_name)
                {
                    Ok(f) => f,
                    Err(_) => return,
                };
                let mut writer = BufWriter::with_capacity(buf_size, file);
                let _ = copy_to_file(
                    &client_local,
                    sql,
                    &mut writer,
                    &total_rows_clone,
                    Duration::from_secs(5),
                    &part_name,
                )
                .await;
                let _ = writer.flush();
            });
            handles.push(handle);
        }
        for h in handles {
            let _ = h.await;
        }
        if opts.gzip {
            use flate2::Compression;
            use flate2::write::GzEncoder;
            use std::fs::{File, OpenOptions};
            use std::io::{BufReader, BufWriter, Read, Write};
            let tmp_merge = {
                let p = Path::new(&final_filename);
                let parent = p.parent().map(|p| p.to_path_buf());
                let stem = p
                    .file_stem()
                    .and_then(|s| s.to_str())
                    .unwrap_or(&final_filename);
                let name = format!("{}.merge.csv", stem);
                let pp = if let Some(parent) = parent {
                    parent.join(name)
                } else {
                    std::path::PathBuf::from(name)
                };
                pp.to_string_lossy().to_string()
            };
            let mut out = BufWriter::with_capacity(
                opts.buf_size_bytes,
                OpenOptions::new()
                    .create(true)
                    .write(true)
                    .truncate(true)
                    .open(&tmp_merge)
                    .context("open tmp merge failed")?,
            );
            for i in 0..parts {
                let part_name = {
                    let p = Path::new(&final_filename);
                    let parent = p.parent().map(|p| p.to_path_buf());
                    let stem = p
                        .file_stem()
                        .and_then(|s| s.to_str())
                        .unwrap_or(&final_filename);
                    let name = format!("{}.part{}.csv", stem, i);
                    let pp = if let Some(parent) = parent {
                        parent.join(name)
                    } else {
                        std::path::PathBuf::from(name)
                    };
                    pp.to_string_lossy().to_string()
                };
                let f = File::open(&part_name).context("open part file failed")?;
                let mut rdr = BufReader::new(f);
                let mut buf = vec![0u8; 32 << 20];
                loop {
                    let n = rdr.read(&mut buf).context("read part file failed")?;
                    if n == 0 {
                        break;
                    }
                    out.write_all(&buf[..n]).context("write merge failed")?;
                }
            }
            out.flush().context("flush merge failed")?;
            let gz_file = OpenOptions::new()
                .create(true)
                .write(true)
                .truncate(true)
                .open(&final_filename)
                .context("open final gzip file failed")?;
            let mut gz = GzEncoder::new(
                BufWriter::with_capacity(opts.buf_size_bytes, gz_file),
                Compression::fast(),
            );
            let mut src =
                BufReader::new(File::open(&tmp_merge).context("open tmp merge for gzip failed")?);
            let mut buf = vec![0u8; 32 << 20];
            loop {
                let n = src.read(&mut buf).context("read tmp merge failed")?;
                if n == 0 {
                    break;
                }
                gz.write_all(&buf[..n]).context("write gzip failed")?;
            }
            gz.flush().context("flush gzip failed")?;
        } else {
            use std::fs::{File, OpenOptions};
            use std::io::{BufReader, BufWriter, Read, Write};
            let mut out = BufWriter::with_capacity(
                opts.buf_size_bytes,
                OpenOptions::new()
                    .create(true)
                    .write(true)
                    .truncate(true)
                    .open(&final_filename)
                    .context("open final file failed")?,
            );
            for i in 0..parts {
                let part_name = {
                    let p = Path::new(&final_filename);
                    let parent = p.parent().map(|p| p.to_path_buf());
                    let stem = p
                        .file_stem()
                        .and_then(|s| s.to_str())
                        .unwrap_or(&final_filename);
                    let ext = p.extension().and_then(|s| s.to_str()).unwrap_or("csv");
                    let name = format!("{}.part{}.{}", stem, i, ext);
                    let pp = if let Some(parent) = parent {
                        parent.join(name)
                    } else {
                        std::path::PathBuf::from(name)
                    };
                    pp.to_string_lossy().to_string()
                };
                let f = File::open(&part_name).context("open part file failed")?;
                let mut rdr = BufReader::new(f);
                let mut buf = vec![0u8; 32 << 20];
                loop {
                    let n = rdr.read(&mut buf).context("read part file failed")?;
                    if n == 0 {
                        break;
                    }
                    out.write_all(&buf[..n])
                        .context("write final file failed")?;
                }
            }
            out.flush().context("flush final file failed")?;
        }
    }

    let count_row = client
        .query_one(&format!("select count(*) from {}", table_full), &[])
        .await
        .context("count query failed")?;
    let db_count: i64 = count_row.get(0);
    let csv_count: usize = if final_filename.ends_with(".gz") {
        use flate2::read::GzDecoder;
        use std::fs::File;
        use std::io::{BufReader, Read};
        let f = File::open(&final_filename).context("open final gzip for counting failed")?;
        let dec = GzDecoder::new(f);
        let mut reader = BufReader::new(dec);
        let mut buf = vec![0u8; 32 << 20];
        let mut scanner = CsvRowScanner::default();
        let mut rows = 0usize;
        loop {
            let n = reader
                .read(&mut buf)
                .context("read gzip for counting failed")?;
            if n == 0 {
                break;
            }
            rows += scanner.count_rows_chunk(&buf[..n]);
        }
        rows
    } else {
        use std::fs::File;
        use std::io::{BufReader, Read};
        let f = File::open(&final_filename).context("open final file for counting failed")?;
        let mut reader = BufReader::new(f);
        let mut buf = vec![0u8; 32 << 20];
        let mut scanner = CsvRowScanner::default();
        let mut rows = 0usize;
        loop {
            let n = reader
                .read(&mut buf)
                .context("read final file for counting failed")?;
            if n == 0 {
                break;
            }
            rows += scanner.count_rows_chunk(&buf[..n]);
        }
        rows
    };
    println!(
        "数据库行数: {}, CSV行数: {}, 是否一致: {}",
        db_count,
        csv_count,
        (db_count as usize) == csv_count
    );
    Ok(())
}

pub async fn pg_transfor_to_csv_async(table_name: &str, filename: &str) -> Result<()> {
    let dsn = "host=localhost port=15432 user=gaussdb password=123456 dbname=yebt_province password=Enmo@123";
    pg_transfor_to_csv_async_with_options_dsn(
        table_name,
        filename,
        CsvExportOptions::default(),
        dsn,
    )
    .await
}

#[derive(Deserialize)]
pub struct PgExportJsonParams {
    pub host: String,
    pub port: String,
    pub username: String,
    pub password: String,
    pub dbname: String,
    pub table_name: String,
    pub current_schema: Option<String>,
    pub output_file_path: String,
    pub gzip: Option<bool>,
    pub buf_size_bytes: Option<usize>,
    pub partitions: Option<usize>,
    pub rows_per_query: Option<usize>,
    pub sslmode: Option<String>,
    pub connect_timeout: Option<u64>,
    pub hostaddr: Option<String>,
    pub dsn: Option<String>,
}

fn build_dsn_from_json(p: &PgExportJsonParams) -> String {
    if let Some(d) = p.dsn.as_ref().filter(|s| !s.is_empty()) {
        return d.clone();
    }
    let mut parts = Vec::new();
    if !p.host.is_empty() {
        parts.push(format!("host={}", p.host));
    }
    if let Some(ha) = p.hostaddr.as_ref().filter(|s| !s.is_empty()) {
        parts.push(format!("hostaddr={}", ha));
    } else if p.host == "localhost" {
        parts.push("hostaddr=127.0.0.1".to_string());
    }
    if !p.port.is_empty() {
        parts.push(format!("port={}", p.port));
    }
    if !p.username.is_empty() {
        parts.push(format!("user={}", p.username));
    }
    if !p.password.is_empty() {
        parts.push(format!("password={}", p.password));
    }
    if !p.dbname.is_empty() {
        parts.push(format!("dbname={}", p.dbname));
    }
    parts.push(format!(
        "sslmode={}",
        p.sslmode.clone().unwrap_or_else(|| "disable".to_string())
    ));
    parts.push(format!(
        "connect_timeout={}",
        p.connect_timeout.unwrap_or(10)
    ));
    parts.join(" ")
}

fn build_table_full(p: &PgExportJsonParams) -> String {
    match p.current_schema.as_ref().filter(|s| !s.is_empty()) {
        Some(s) => format!("\"{}\".\"{}\"", s, p.table_name),
        None => p.table_name.clone(),
    }
}

pub async fn pg_export_from_json(json: &str) -> Result<()> {
    let params: PgExportJsonParams = serde_json::from_str(json).context("invalid json")?;
    let dsn = build_dsn_from_json(&params);
    let table_full = build_table_full(&params);
    let mut opts = CsvExportOptions::default();
    if let Some(g) = params.gzip {
        opts.gzip = g;
    }
    if let Some(b) = params.buf_size_bytes {
        if b > 0 {
            opts.buf_size_bytes = b;
        }
    }
    if let Some(p) = params.partitions {
        if p > 0 {
            opts.partitions = p;
        }
    }
    if let Some(r) = params.rows_per_query {
        if r > 0 {
            opts.rows_per_query = Some(r);
        }
    }
    pg_transfor_to_csv_async_with_options_dsn(&table_full, &params.output_file_path, opts, &dsn)
        .await
}
struct CsvRowScanner {
    in_quotes: bool,
    pending_quote: bool,
}

impl Default for CsvRowScanner {
    fn default() -> Self {
        Self {
            in_quotes: false,
            pending_quote: false,
        }
    }
}

impl CsvRowScanner {
    fn count_rows_chunk(&mut self, chunk: &[u8]) -> usize {
        let mut rows = 0usize;
        let mut i = 0usize;
        while i < chunk.len() {
            let b = chunk[i];
            if self.pending_quote {
                if b == b'"' {
                    self.pending_quote = false;
                } else {
                    self.in_quotes = false;
                    self.pending_quote = false;
                    if b == b'\n' && !self.in_quotes {
                        rows += 1;
                    }
                }
                i += 1;
                continue;
            }
            if b == b'"' {
                if self.in_quotes {
                    self.pending_quote = true;
                } else {
                    self.in_quotes = true;
                }
            } else if b == b'\n' {
                if !self.in_quotes {
                    rows += 1;
                }
            }
            i += 1;
        }
        rows
    }
}

/// 测试转换CSV文件
#[test]
pub fn run_test() {
    let table_name = "yebt_apply_record";
    let filename = format!("C:\\Users\\13472\\Desktop\\{}.csv", table_name);
    let params = serde_json::json!({
        "host": "localhost",
        "port": "15432",
        "username": "gaussdb",
        "password": "Enmo@123",
        "dbname": "yebt_province",
        "table_name": table_name,
        "current_schema": "public",
        "output_file_path": filename,
        "gzip": false,
        "buf_size_bytes": 134_217_728,
        "partitions": 1,
        "rows_per_query": 0
    });
    let ret = tokio::runtime::Runtime::new()
        .expect("Failed to create Tokio runtime")
        .block_on(pg_export_from_json(&params.to_string()));
    if let Err(e) = ret {
        println!("转换CSV文件失败: {}", e);
    } else {
        println!("转换CSV文件成功: {}", filename);
    }
}
