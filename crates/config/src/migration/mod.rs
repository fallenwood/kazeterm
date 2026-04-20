use toml::Value;

mod steps;
use steps::*;

/// Current config version in YYYYMMDD.Rev format.
pub const CURRENT_CONFIG_VERSION: &str = "20260419.1";

/// A migration that transforms raw TOML config from one version to the next.
struct Migration {
  from_version: &'static str,
  to_version: &'static str,
  migrate: fn(&mut Value),
}

/// Registry of all migrations, ordered from oldest to newest.
/// Each migration transforms the config from `from_version` to `to_version`.
/// To add a new migration:
/// 1. Add a new entry at the end of this list
/// 2. Set `from_version` to the previous `CURRENT_CONFIG_VERSION`
/// 3. Set `to_version` to the new version
/// 4. Update `CURRENT_CONFIG_VERSION` to the new version
/// 5. Implement the migration function that modifies the raw TOML `Value`
fn migrations() -> &'static [Migration] {
  &[
    Migration {
      from_version: "0",
      to_version: "20260208.1",
      migrate: migrate_v0_to_20260208_1,
    },
    Migration {
      from_version: "20260208.1",
      to_version: "20260220.1",
      migrate: migrate_v20260208_1_to_20260220_1,
    },
    Migration {
      from_version: "20260220.1",
      to_version: "20260303.1",
      migrate: migrate_v20260220_1_to_20260303_1,
    },
    Migration {
      from_version: "20260303.1",
      to_version: "20260306.1",
      migrate: migrate_v20260303_1_to_20260306_1,
    },
    Migration {
      from_version: "20260306.1",
      to_version: "20260322.1",
      migrate: migrate_v20260306_1_to_20260322_1,
    },
    Migration {
      from_version: "20260322.1",
      to_version: "20260323.1",
      migrate: migrate_v20260322_1_to_20260323_1,
    },
    Migration {
      from_version: "20260323.1",
      to_version: "20260323.2",
      migrate: migrate_v20260323_1_to_20260323_2,
    },
    Migration {
      from_version: "20260323.2",
      to_version: "20260327.1",
      migrate: migrate_v20260323_2_to_20260327_1,
    },
    Migration {
      from_version: "20260327.1",
      to_version: "20260407.1",
      migrate: migrate_v20260327_1_to_20260407_1,
    },
    Migration {
      from_version: "20260407.1",
      to_version: "20260411.1",
      migrate: migrate_v20260407_1_to_20260411_1,
    },
    Migration {
      from_version: "20260411.1",
      to_version: "20260411.2",
      migrate: migrate_v20260411_1_to_20260411_2,
    },
    Migration {
      from_version: "20260411.2",
      to_version: "20260411.3",
      migrate: migrate_v20260411_2_to_20260411_3,
    },
    Migration {
      from_version: "20260411.3",
      to_version: "20260412.1",
      migrate: migrate_v20260411_3_to_20260412_1,
    },
    Migration {
      from_version: "20260412.1",
      to_version: "20260412.2",
      migrate: migrate_v20260412_1_to_20260412_2,
    },
    Migration {
      from_version: "20260412.2",
      to_version: "20260412.3",
      migrate: migrate_v20260412_2_to_20260412_3,
    },
    Migration {
      from_version: "20260412.3",
      to_version: "20260414.1",
      migrate: migrate_v20260412_3_to_20260414_1,
    },
    Migration {
      from_version: "20260414.1",
      to_version: "20260414.2",
      migrate: migrate_v20260414_1_to_20260414_2,
    },
    Migration {
      from_version: "20260414.2",
      to_version: "20260415.1",
      migrate: migrate_v20260414_2_to_20260415_1,
    },
    Migration {
      from_version: "20260415.1",
      to_version: "20260415.2",
      migrate: migrate_v20260415_1_to_20260415_2,
    },
    Migration {
      from_version: "20260415.2",
      to_version: "20260415.3",
      migrate: migrate_v20260415_2_to_20260415_3,
    },
    Migration {
      from_version: "20260415.3",
      to_version: "20260416.1",
      migrate: migrate_v20260415_3_to_20260416_1,
    },
    Migration {
      from_version: "20260416.1",
      to_version: "20260416.2",
      migrate: migrate_v20260416_1_to_20260416_2,
    },
    Migration {
      from_version: "20260416.2",
      to_version: "20260416.3",
      migrate: migrate_v20260416_2_to_20260416_3,
    },
    Migration {
      from_version: "20260416.3",
      to_version: "20260417.1",
      migrate: migrate_v20260416_3_to_20260417_1,
    },
    Migration {
      from_version: "20260417.1",
      to_version: "20260417.2",
      migrate: migrate_v20260417_1_to_20260417_2,
    },
    Migration {
      from_version: "20260417.2",
      to_version: "20260417.3",
      migrate: migrate_v20260417_2_to_20260417_3,
    },
    Migration {
      from_version: "20260417.3",
      to_version: "20260419.1",
      migrate: migrate_v20260417_3_to_20260419_1,
    },
  ]
}

/// Apply all necessary migrations to bring the config up to `CURRENT_CONFIG_VERSION`.
/// Returns `true` if any migrations were applied, `false` if the config was already current.
pub fn apply_migrations(value: &mut Value) -> bool {
  let current_version = value
    .get("version")
    .and_then(|v| v.as_str())
    .unwrap_or("0")
    .to_string();

  if current_version == CURRENT_CONFIG_VERSION {
    return false;
  }

  let all_migrations = migrations();

  // Find the starting migration index
  let start_idx = match all_migrations
    .iter()
    .position(|m| m.from_version == current_version)
  {
    Some(idx) => idx,
    None => {
      tracing::warn!(
        "Unknown config version '{}', attempting to use as-is",
        current_version
      );
      return false;
    }
  };

  // Apply migrations in sequence
  for migration in &all_migrations[start_idx..] {
    tracing::info!(
      "Migrating config from {} to {}",
      migration.from_version,
      migration.to_version
    );
    (migration.migrate)(value);
  }

  true
}

#[cfg(test)]
mod tests;
