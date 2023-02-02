use std::error::Error;

use vergen::{vergen, Config, SemverKind, ShaKind};

fn main() -> Result<(), Box<dyn Error>> {
    let mut config = Config::default();
    // Change the SHA output to the short variant
    *config.git_mut().sha_kind_mut() = ShaKind::Short;
    *config.git_mut().semver_kind_mut() = SemverKind::Lightweight;
    *config.git_mut().semver_dirty_mut() = Some("-dirty");
    // Generate the instructions
    vergen(config)?;
    Ok(())
}
