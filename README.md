# RoboKitty

RoboKitty is a dual-interface budget management system that helps organizations manage team participation, proposal voting, and reward distribution. It provides both a CLI and a Telegram bot interface for easy interaction.

## Features

- Team management with Earner/Supporter status tracking
- Proposal lifecycle management
- Fair participation selection through Ethereum-based raffles
- Formal and informal voting mechanisms
- Automated point tracking and reward distribution
- Comprehensive reporting system
- Data stored in .JSON
- Secure state persistence

## Installation

### Prerequisites

- Rust (latest stable version)
- Access to an Ethereum node (via IPC)
- Telegram bot token (for bot functionality)

### Building and Installation

1. Build the project:
```bash
git clone https://github.com/your-org/robokitty.git
cd robokitty
cargo build --release
```

2. Create a dedicated directory for the application:
```bash
sudo mkdir -p /opt/robokitty
```

3. Copy the binaries:
```bash
sudo cp target/release/robokitty_cli /opt/robokitty/
sudo cp target/release/robokitty_bot /opt/robokitty/
```

4. Set up configuration:
```bash
# Copy and edit configuration files
cp .env.template /opt/robokitty/.env
cp config.toml.template /opt/robokitty/config.toml
cd /opt/robokitty
```

Alternatively, for development/testing:
```bash
# Create a local test environment
mkdir ~/robokitty-test
cp target/release/robokitty_* ~/robokitty-test/
cp .env.template ~/robokitty-test/.env
cp config.toml.template ~/robokitty-test/config.toml
cd ~/robokitty-test
```

## Configuration

1. Edit `.env` to set your Telegram bot token:
```
TELEGRAM_BOT_TOKEN=your_token_here
```

2. Configure `config.toml` with your settings:
```toml
ipc_path = "/path/to/ethereum/node.ipc"
future_block_offset = 2
state_file = "budget_system_state.json"
script_file = "input_script.json"
default_total_counted_seats = 7
default_max_earner_seats = 5
default_qualified_majority_threshold = 0.7
counted_vote_points = 5
uncounted_vote_points = 2
```

Note: Both `.env` and `config.toml` must be in the same directory as the binaries.

## Usage

### CLI Interface

```bash
# Create a new epoch
./robokitty_cli create-epoch "Q1 2024" "2024-01-01" "2024-03-31"

# Add a team
./robokitty_cli add-team "Team Alpha" "John Doe" 1000,2000,3000

# Create a proposal
./robokitty_cli add-proposal "New Initiative" "https://example.com/proposal"

# Run a raffle
./robokitty_cli create-raffle "New Initiative"

# Process votes
./robokitty_cli create-and-process-vote "New Initiative" "Team1:Yes,Team2:No" "Team3:Yes"
```

### Telegram Bot

Start the bot:
```bash
./robokitty_bot
```

Available commands in Telegram:
- `/help` - Display available commands
- `/print_team_report` - Show team information
- `/print_epoch_state` - Show current epoch status
- `/create_epoch` - Create a new epoch
- `/add_team` - Add a new team
- `/create_raffle` - Create a new raffle
And more...

## Security Considerations

- Keep your `.env` and `config.toml` files secure and never commit them to version control
- The system uses file locking to prevent concurrent modifications
- All user inputs are validated before processing
- State files are kept in a secure location with appropriate permissions
- Ensure proper file permissions are set on configuration files and binaries

## Development

### Running Tests

```bash
cargo test
```

### Code Structure

- `src/core/`: Core business logic and data models
- `src/services/`: External service integrations (Ethereum, Telegram)
- `src/commands/`: Command processing for CLI and Telegram
- `src/bin/`: Binary entry points

## Contributing

Contributions are welcome! Please follow these guidelines:

1. Fork the repository
2. Create a feature branch (`git checkout -b feature/amazing-feature`)
3. Make your changes
4. Run the test suite to ensure everything works
5. Commit your changes (`git commit -m 'Add amazing feature'`)
6. Push to your branch (`git push origin feature/amazing-feature`)
7. Open a Pull Request

Please ensure your contributions:
- Include tests for new functionality
- Follow the existing code style
- Update documentation as needed
- Do not include any sensitive information
- Are appropriately licensed (see below)

## License

This project is licensed under the GNU Affero General Public License v3.0 (AGPL-3.0).

This means:
- You can use, modify, and distribute this software
- If you modify the software and run it on a server, you must release your modifications
- Any derivative work must also be licensed under AGPL-3.0
- See the [LICENSE](LICENSE) file for details