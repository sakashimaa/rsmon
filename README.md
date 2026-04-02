## rsmon

<img width="1668" height="1398" alt="image" src="https://github.com/user-attachments/assets/6f8fd4e9-d089-4a53-8685-e6476e66703c" />

Lightweight terminal system monitor for Linux.

![screenshot](screenshot.png)

### Features

- Memory & CPU usage with dynamic color thresholds
- CPU history sparkline (60s)
- Process list sorted by memory usage
- Process filtering and kill support

### Controls

| Key   | Action           |
| ----- | ---------------- |
| `j/k` | Scroll processes |
| `/`   | Filter by name   |
| `d`   | Kill process     |
| `Esc` | Clear filter     |
| `q`   | Quit             |

### Installation

```bash
curl -fsSL https://raw.githubusercontent.com/sakashimaa/rsmon/main/install.sh | sh
```

### Requirements

- Linux (uses `/proc` filesystem)
- Rust toolchain
