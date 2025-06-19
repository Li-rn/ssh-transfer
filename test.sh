#!/bin/bash

# 编译项目
cargo build --release

# 显示帮助信息
echo "=== 显示帮助信息 ==="
./target/release/ssh-transfer --help

echo -e "\n=== 工具编译成功！==="
echo "使用示例："
echo "下载文件: ./target/release/ssh-transfer -H server.com -u username download /remote/file.txt ./local/file.txt"
echo "上传文件: ./target/release/ssh-transfer -H server.com -u username upload ./local/file.txt /remote/file.txt"
echo "递归下载: ./target/release/ssh-transfer -H server.com -u username download -r /remote/dir ./local/dir"
echo "使用SSH密钥: ./target/release/ssh-transfer -H server.com -u username -k ~/.ssh/id_rsa download /remote/file.txt ./local/file.txt"
