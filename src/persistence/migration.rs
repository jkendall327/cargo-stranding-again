use super::{Save, SaveVersion, CURRENT_SAVE_VERSION};

const INITIAL_SAVE_VERSION: SaveVersion = SaveVersion::new(1);

/// Error returned when a save envelope cannot be migrated to the current schema.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SaveMigrationError {
    UnsupportedVersion { version: SaveVersion },
}

/// Migrates a typed save envelope to `CURRENT_SAVE_VERSION`.
///
/// Version 2 is intentionally payload-compatible with version 1. Keeping this
/// explicit no-op gives future schema changes a concrete place to preserve old
/// saves without forcing current structs to stay shaped like history forever.
pub fn migrate_save<T>(mut save: Save<T>) -> Result<Save<T>, SaveMigrationError> {
    match save.metadata.version {
        INITIAL_SAVE_VERSION => {
            save.metadata.version = CURRENT_SAVE_VERSION;
            Ok(save)
        }
        CURRENT_SAVE_VERSION => Ok(save),
        version => Err(SaveMigrationError::UnsupportedVersion { version }),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::persistence::{SaveKind, SaveMetadata};

    #[test]
    fn v1_save_migrates_to_current_version_without_touching_payload() {
        let save = Save {
            metadata: SaveMetadata {
                version: SaveVersion::new(1),
                kind: SaveKind::World,
            },
            payload: "payload",
        };

        let migrated = migrate_save(save).expect("v1 should migrate to current");

        assert_eq!(migrated.metadata.version, CURRENT_SAVE_VERSION);
        assert_eq!(migrated.metadata.kind, SaveKind::World);
        assert_eq!(migrated.payload, "payload");
    }

    #[test]
    fn unknown_save_version_fails_loudly() {
        let save = Save {
            metadata: SaveMetadata {
                version: SaveVersion::new(CURRENT_SAVE_VERSION.0 + 1),
                kind: SaveKind::World,
            },
            payload: (),
        };

        assert_eq!(
            migrate_save(save),
            Err(SaveMigrationError::UnsupportedVersion {
                version: SaveVersion::new(CURRENT_SAVE_VERSION.0 + 1)
            })
        );
    }
}
