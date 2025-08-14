# Contributing to RoboKitty

Thank you for your interest in contributing to RoboKitty! Whether you're fixing a bug, adding a feature, or improving documentation, your help is appreciated. This guide will help you get started.

## Setting Up a Development Environment

1.  **Prerequisites:**
    -   Install the [Rust toolchain](https://rustup.rs/).
    -   Clone the repository: `git clone https://github.com/<your-username>/robokitty.git`
    -   (Optional but Recommended) Set up a local Ethereum node like [Reth](https://github.com/paradigmxyz/reth) for running tests that require IPC access.

2.  **Build the Project:**
    ```bash
    cargo build
    ```

3.  **Run Tests:**
    To ensure everything is set up correctly, run the full test suite:
    ```bash
    cargo test --all-features
    ```

## Codebase Structure

The RoboKitty codebase is organized into several logical modules to separate concerns.

-   `src/bin/`: Contains the entry points for the two binaries.
    -   `robokitty_cli.rs`: The main function and argument parsing for the Command-Line Interface.
    -   `robokitty_bot.rs`: The main function and setup for the Telegram Bot.

-   `src/app_config.rs`: Defines the `AppConfig` struct and handles loading all settings from `config.toml` and environment variables.

-   `src/commands/`: This module acts as the "frontend" for all operations.
    -   `common.rs`: Defines the central `Command` enum and shared data structures like `UpdateProposalDetails`. All external interfaces are translated into these common commands.
    -   `cli.rs`: Uses the `clap` crate to parse command-line arguments and translate them into the common `Command` enum.
    -   `telegram.rs`: Uses the `teloxide` crate to define bot commands and translate them into the common `Command` enum.

-   `src/core/`: The "backend" and heart of the application logic.
    -   `budget_system.rs`: The central engine. It holds the application state and contains all the business logic for executing commands (e.g., `create_team`, `add_proposal`, `prepare_raffle`). This is where most of the core logic lives.
    -   `models/`: Defines the core data structures of the application: `Team`, `Epoch`, `Proposal`, `Raffle`, and `Vote`. These structs are designed to be serializable to/from JSON.
    -   `state.rs`: Defines the main `BudgetSystemState` struct, which is the top-level container for all data that gets saved to `budget_system_state.json`.
    -   `reporting.rs`: Contains all logic for calculating statistics and formatting the final markdown reports.
    -   `file_system.rs`: Handles the serialization (saving) and deserialization (loading) of the `BudgetSystemState` to the JSON file.

-   `src/services/`: Contains modules for interacting with external services.
    -   `ethereum.rs`: Defines the `EthereumServiceTrait` and its implementations (`EthereumService` for live IPC and `MockEthereumService` for testing). It handles all communication with an Ethereum node.
    -   `telegram.rs`: Contains the logic for running the `teloxide` bot framework, handling incoming messages, and dispatching commands to the `BudgetSystem`.

-   `src/lock.rs`: Implements a simple file-based lock (`robokitty.lock`) to prevent race conditions where the CLI and the bot might try to modify the state file simultaneously.

## Contribution Workflow

1.  **Find an Issue:** Look for open issues in the issue tracker. If you have a new idea, please open an issue first to discuss it with the maintainers.

2.  **Fork the Repository:** Create your own fork of the RoboKitty repository.

3.  **Create a Branch:** Create a new branch for your feature or bugfix.
    ```bash
    git checkout -b feature/my-new-feature
    ```

4.  **Make Your Changes:**
    -   Write clean, readable code.
    -   Add comments where necessary to explain complex logic.
    -   Ensure your changes are covered by new or existing tests.

5.  **Format and Test:** Before committing, make sure your code is formatted and passes all tests.
    ```bash
    cargo fmt
    cargo test --all-features
    ```

6.  **Commit Your Changes:** Write a clear and descriptive commit message.
    ```bash
    git commit -m "feat: Add support for multi-token budget requests"
    ```

7.  **Push to Your Fork:**
    ```bash
    git push origin feature/my-new-feature
    ```

8.  **Open a Pull Request:** Go to the original RoboKitty repository and open a pull request from your fork. Provide a detailed description of your changes in the PR description.

A maintainer will review your PR, and once it's approved, it will be merged. Thank you for your contribution