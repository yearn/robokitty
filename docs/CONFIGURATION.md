# RoboKitty Configuration Guide (`config.toml`)

The `config.toml` file is the primary way to configure the behavior of your RoboKitty instance. This guide explains each parameter in detail.

## [General Settings]

### `ipc_path`
-   **Type:** String
-   **Default:** `"/tmp/reth.ipc"`
-   **Description:** The absolute file path to your Ethereum node's IPC (Inter-Process Communication) socket file. RoboKitty uses this to communicate directly with the node to fetch block numbers and randomness (`mixHash`). Ensure the user running RoboKitty has read permissions for this file.

### `future_block_offset`
-   **Type:** Integer
-   **Default:** `10`
-   **Description:** This is a crucial security parameter for the raffle mechanism. When a raffle is initiated, RoboKitty targets a block `N` blocks in the future for its source of randomness. This prevents any party (including the bot operator) from predicting the raffle outcome. A higher number increases the time to finalize a raffle but provides stronger security against short-term block hash manipulation.

### `state_file`
-   **Type:** String
-   **Default:** `"budget_system_state.json"`
-   **Description:** The path to the JSON file where RoboKitty's entire state is stored. This includes all information about teams, epochs, proposals, votes, and raffles. It can be a relative path (from where you run the binary) or an absolute path. **It is critical to back up this file regularly.**

### `script_file`
-   **Type:** String
-   **Default:** `"input_script.json"`
-   **Description:** The default file path used by the `robokitty_cli run-script` command if no specific path is provided as an argument. This is useful for automating a standard sequence of setup commands.

---

## [Governance & Voting Settings]

### `default_total_counted_seats`
-   **Type:** Integer
-   **Default:** `7`
-   **Description:** The default number of "counted" voter seats that will be selected in a formal voting raffle. The votes from these teams determine the pass/fail outcome of a proposal.

### `default_max_earner_seats`
-   **Type:** Integer
-   **Default:** `5`
-   **Description:** The default maximum number of the `default_total_counted_seats` that can be allocated to 'Earner' status teams. This ensures that 'Supporter' teams have a guaranteed minimum representation among counted voters. This value cannot be greater than `default_total_counted_seats`.

### `default_qualified_majority_threshold`
-   **Type:** Float (between 0.0 and 1.0)
-   **Default:** `0.7`
-   **Description:** The proportion of "counted" voters who must vote 'Yes' for a formal proposal to be approved. A value of `0.7` means 70% of the counted seats must vote 'Yes'.

### `counted_vote_points`
-   **Type:** Integer
-   **Default:** `5`
-   **Description:** The number of participation points awarded to a team for casting a vote as a "counted" voter in a formal vote. These points are used to calculate reward distributions at the end of an epoch.

### `uncounted_vote_points`
-   **Type:** Integer
-   **Default:** `2`
-   **Description:** The number of participation points awarded to a team for casting a vote as an "uncounted" voter in a formal vote. This incentivizes all teams to participate, even if they are not selected for the counted group.

---

## [Telegram Settings]

### `telegram.chat_id`
-   **Type:** String
-   **Default:** `""`
-   **Description:** The unique identifier for the Telegram chat (group or channel) where the bot will operate. You can find this ID by using a bot like `@userinfobot`. For groups, this will be a negative number.