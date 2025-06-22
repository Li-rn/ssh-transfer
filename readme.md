# SSH Transfer Tool

A high-performance, multi-threaded SSH file transfer tool with resume capability.

## Features

- Multi-threaded parallel transfers
- Resume interrupted downloads/uploads
- Recursive directory operations
- Progress tracking
- Support for SSH key authentication
- Configurable chunk sizes and thread counts

## Environment

**安装 Rust 工具链**

macOS/Linux：

```bash
curl https://sh.rustup.rs -sSf | sh
```

Windows：[https://rustup.rs/](https://rustup.rs/)

## Installation

```bash
cargo build --release
```

## Manual

=== 帮助信息 ===

**A multi-threaded SSH file transfer tool**

**Usage**: **ssh-transfer** [OPTIONS] **--host** `<HOST>` **--username** `<USERNAME>` `<COMMAND>`

**Commands:**

  **download**  Download files from remote server

  **upload**    Upload files to remote server

  **help**      Print this message or the help of the given subcommand(s)

**Options:**

  **-H**, **--host** `<HOST>`              SSH server hostname or IP address

  **-p**, **--port** `<PORT>`              SSH server port [default: 22]

  **-u**, **--username** `<USERNAME>`      SSH username

  **-P**, **--password** `<PASSWORD>`      SSH password (if not provided, will prompt for input)

  **-k**, **--key-file** <KEY_FILE>      SSH private key file path
        **--use-agent**                Use SSH agent for authentication

  **-t**,  **--threads** `<THREADS>`        Number of parallel threads [default: 4]

  **-c**, **--chunk-size** <CHUNK_SIZE>  Chunk size in bytes [default: 1048576]

  **-r**, **--resume**                   Enable resume capability

  **-v**, **--verbose**                  Verbose output

  **-h**, **--help**                     Print help

  **-V**, **--version**                  Print version

=== 使用示例 ===

**下载文件:** ./target/release/ssh-transfer -H server.com -u username download /remote/file.txt ./local/file.txt

**上传文件:** ./target/release/ssh-transfer -H server.com -u username upload ./local/file.txt /remote/file.txt

**递归下载:** ./target/release/ssh-transfer -H server.com -u username download -r /remote/dir ./local/dir

**使用SSH密钥:** ./target/release/ssh-transfer -H server.com -u username -k ~/.ssh/id_rsa download /remote/file.txt ./local/file.txt
