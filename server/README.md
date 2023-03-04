# Server
A terminal-based server implementing the chat-rs protocol, with logging via the `env_logger` crate.

## Usage:
By default, the server binds to `0.0.0.0`. To change the address, provide it as a command-line argument at startup.  
The server currently doesn't support a different port than `7878`.

To change the log level, set the `RUST_LOG` environment variable accordingly - possible values are
* `error`
* `warn`
* `info`
* `debug`
* `trace`

The server operates in encrypted mode by default - to disable encrypted mode, set the environment variable `CHAT_RS_UNENCRYPTED`.

Currently, the server has a hard-coded limit of 50 connected users.

---
![image](https://user-images.githubusercontent.com/33005025/152642207-1be3552e-f2ff-4054-a3ed-4a0115faa59b.png)
