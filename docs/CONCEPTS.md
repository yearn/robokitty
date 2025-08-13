# RoboKitty Core Concepts

Understanding these core concepts is key to using RoboKitty effectively and interpreting its outputs correctly.

## State (`budget_system_state.json`)

RoboKitty is a stateful application. The entire configuration of teams, epochs, proposals, votes, and raffles is serialized and stored in a single JSON file, defined by the `state_file` parameter in your `config.toml`.

This file is the single source of truth for the application. Every command you run reads from and writes to this file.

**Key Implications:**
-   **Persistence:** You can stop and restart the CLI or Telegram bot at any time without losing data.
-   **Backup is Critical:** You are responsible for backing up this file. Losing it means losing all historical data and system configuration.
-   **Concurrency:** To prevent data corruption, a `robokitty.lock` file is created whenever an operation is modifying the state file. This prevents the CLI and the bot from writing to the file at the same time.

## Epochs

An Epoch is a defined time period, similar to a financial quarter. It serves as a container for governance and budget activities.

-   **Lifecycle:** An epoch progresses through three statuses:
    1.  `Planned`: The epoch has been created but is not yet active. Its dates can still be modified.
    2.  `Active`: The epoch is currently running. New proposals are associated with this epoch. Only one epoch can be active at a time.
    3.  `Closed`: The epoch has ended. No new proposals can be added. At this stage, final reward calculations are performed.
-   **Rewards:** An epoch can have a total reward pool (e.g., `18 YFI`). At the end of the epoch, this pool is distributed to participating teams based on the governance points they earned.

## Teams

Teams are the core actors in the system. They submit proposals and participate in votes.

-   **`Earner` Status:** An Earner team is one that generates direct revenue for the DAO. Their "weight" in a voting raffle is influenced by their `trailing_monthly_revenue`. This gives teams with a larger economic impact a greater chance of being selected as a "counted" voter.
-   **`Supporter` Status:** A Supporter team does not generate direct revenue but provides essential services (e.g., support, security, governance). They have a baseline chance of being selected in a raffle, ensuring their voice is heard.
-   **`Inactive` Status:** An Inactive team cannot participate in governance activities.

## The Raffle and Voting Mechanism

This is the most critical and sophisticated feature of RoboKitty. It ensures that formal proposal voting is both fair and resistant to manipulation.

1.  **Initiation:** When a raffle is created for a proposal, the system records the current Ethereum block number (e.g., block `1,000,000`).

2.  **Targeting a Future Block:** The system targets a future block for its source of randomness. This is calculated as `current_block + future_block_offset` (e.g., `1,000,000 + 10 = 1,000,010`).

3.  **Waiting:** The system pauses and waits for the Ethereum network to mine blocks up to and including the target block. This delay is essential; it makes it impossible for anyone to know the outcome of the raffle beforehand.

4.  **Fetching Randomness:** Once the target block is confirmed, RoboKitty fetches its `mixHash`. This is a value from the block's header that is computationally difficult to predict in advance, serving as a good source of on-chain randomness.

5.  **Assigning Scores:** This `mixHash` is used as a seed to a deterministic algorithm that assigns a score to every "ticket" held by active teams.
    -   Supporter teams get 1 ticket.
    -   Earner teams get a number of tickets proportional to the square root of their average quarterly revenue, ensuring diminishing returns for extremely high revenue.

6.  **Selecting Voters:** The teams holding the highest-scoring tickets are selected as **"counted" voters**. The number of seats is defined by `default_total_counted_seats` and `default_max_earner_seats`. All other participating teams become **"uncounted" voters**.

7.  **Determining the Outcome:** In a formal vote, only the votes from the "counted" teams are used to determine if a proposal passes the `default_qualified_majority_threshold`.

8.  **Awarding Points:** To incentivize participation from all teams, points are awarded to everyone who votes, but "counted" voters receive more (`counted_vote_points`) than "uncounted" voters (`uncounted_vote_points`). These points are tallied at the end of an epoch to determine reward distributions.
