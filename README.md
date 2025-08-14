# RoboKitty: DAO Budget & Governance System

[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](https://opensource.org/licenses/MIT)
[![Rust](https://github.com/user/robokitty/actions/workflows/rust.yml/badge.svg)](https://github.com/user/robokitty/actions/workflows/rust.yml)
[![Crates.io](https://img.shields.io/crates/v/robokitty.svg)](https://crates.io/crates/robokitty)

RoboKitty is a sophisticated budget and governance management system designed for DAOs and decentralized teams. It provides a robust framework for managing financial epochs, handling budget proposals, conducting fair and verifiable voting raffles using Ethereum block data, and generating detailed performance reports.

The system is accessible through a powerful **Command-Line Interface (CLI)** for administrators and a user-friendly **Telegram Bot** for convenient, on-the-go interactions.

## Key Features

-   **Epoch-Based Budgeting:** Organize financial activities into distinct time periods (Epochs) with their own rewards and proposals.
-   **Proposal Management:** A complete lifecycle for proposals from announcement and publication to resolution (Approved, Rejected, etc.).
-   **On-Chain Raffle System:** A fair and transparent mechanism to select "counted" voters for formal proposals, using future Ethereum block randomness (`mixHash`) to prevent manipulation.
-   **Team & Participation Tracking:** Manage teams, their status (Earner, Supporter), and track their participation in governance, awarding points for engagement.
-   **Comprehensive Reporting:** Generate detailed reports on team performance, epoch summaries, proposal outcomes, unpaid budget requests, and end-of-epoch payment distributions.
-   **Stateful & Persistent:** The entire system state is saved to a single JSON file, allowing it to be stopped and restarted without losing data.

---

## Getting Started

Follow these steps to get your RoboKitty instance up and running.

### Prerequisites

-   **Rust Toolchain:** Install Rust via [rustup.rs](https://rustup.rs/).
-   **Ethereum Node:** RoboKitty requires access to an Ethereum node's IPC file for its raffle mechanism. A local node like [Reth](https://github.com/paradigmxyz/reth) is recommended.
-   **Telegram Bot Token:** Create a bot via Telegram's [@BotFather](https://t.me/botfather) to get a token.

### Installation & Setup

1.  **Clone the Repository:**
    ```bash
    git clone https://github.com/<your-username>/robokitty.git
    cd robokitty
    ```

2.  **Build the Binaries:**
    ```bash
    cargo build --release
    ```
    This compiles two executables in `./target/release/`: `robokitty_cli` and `robokitty_bot`.

3.  **Create Your Configuration:**
    -   **Environment File:** Copy the template to `.env` and add your Telegram bot token.
        ```bash
        cp .env.template .env
        ```
        Then, edit `.env`:
        ```
        TELEGRAM_BOT_TOKEN=123456:ABC-DEF1234ghIkl-zyx57W2v1u123ew11
        ```

    -   **Application Config:** Copy the template to `config.toml`.
        ```bash
        cp config.toml.template config.toml
        ```
        Open `config.toml` and ensure the `ipc_path` points to your Ethereum node's IPC file. You can also configure other system parameters here.

4.  **Run the Application:**
    -   **To use the CLI:**
        ```bash
        ./target/release/robokitty_cli --help
        ```
    -   **To start the Telegram Bot:**
        ```bash
        ./target/release/robokitty_bot
        ```

---

## Documentation

For more detailed information, please refer to our full documentation:

-   **[Configuration Guide](./docs/CONFIGURATION.md):** A deep dive into all the settings in `config.toml`.
-   **[Usage Guide](./docs/USAGE_GUIDE.md):** Comprehensive instructions and examples for both the CLI and the Telegram Bot.
-   **[Core Concepts](./docs/CONCEPTS.md):** An explanation of key ideas like Epochs, Teams, and the Raffle Mechanism.
-   **[Developer & Contribution Guide](./CONTRIBUTING.md):** Information on the codebase structure and how to contribute to the project.

## License

This project is licensed under the MIT License. See the [LICENSE](LICENSE) file for details.