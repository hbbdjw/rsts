use crate::modules::ssh::{SshClient, SshCredentials, SshResult};

// SSH客户端使用示例
#[allow(dead_code)]
pub async fn ssh_client_example() -> SshResult<()> {
    // 创建SSH客户端
    let mut client = SshClient::new();

    // 准备连接凭证
    let credentials = SshCredentials {
        hostname: "192.168.1.100".to_string(),
        port: 22,
        username: "user".to_string(),
        password: "password".to_string(),
    };

    // 连接到SSH服务器
    client.connect(credentials).await?;

    // 执行远程命令
    let result = client.execute_command("ls -la").await?;
    println!("命令执行结果:\n{}", result);

    // 上传文件示例
    // client.upload_file("./local.txt", "/home/user/remote.txt").await?;

    // 下载文件示例
    // client.download_file("/home/user/remote.txt", "./downloaded.txt").await?;

    // 列出目录内容示例
    // let files = client.list_directory("/home/user").await?;
    // println!("目录内容:");
    // for file in files {
    //     println!("- {:<20} [{}] {}", file.name, file.size, if file.is_dir { "目录" } else { "文件" });
    // }

    // 断开连接
    client.disconnect().await?;

    Ok(())
}

// 批量执行命令示例
#[allow(dead_code)]
pub async fn batch_commands_example() -> SshResult<()> {
    let mut client = SshClient::new();

    // 准备连接凭证（实际使用时应从配置或环境变量中获取）
    let credentials = SshCredentials {
        hostname: "example.com".to_string(),
        port: 22,
        username: "admin".to_string(),
        password: "secure_password".to_string(),
    };

    // 连接到服务器
    client.connect(credentials).await?;

    // 定义要执行的命令列表
    let commands = ["uname -a", "df -h", "free -m", "whoami"];

    // 依次执行命令
    for command in commands.iter() {
        println!("\n执行命令: {}", command);
        let result = client.execute_command(command).await?;
        println!("结果:\n{}", result);
    }

    // 断开连接
    client.disconnect().await?;

    Ok(())
}
