#!/bin/bash
# 导航到项目目录
echo "导航到项目目录..."
cd /mnt/f/yuanma/rust/rsts

# 创建输出目录
mkdir -p target/builds/linux

# 构建项目
echo "开始构建项目..."
cargo build --release

# 检查构建是否成功
if [ $? -eq 0 ]; then
    echo "项目构建成功！"
    # 复制可执行文件
echo "将可执行文件复制到输出目录..."
    cp ./target/release/rsts ./target/builds/linux/
    cp -r static ./target/builds/linux/
    cp config.toml ./target/builds/linux/
    cp -r db ./target/builds/linux/
    echo "构建产物已复制完成"
else
    echo "项目构建失败！"
    exit 1
fi
