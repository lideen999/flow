use std::path::Path;

use tokio::process::Command;

use crate::config::settings;

pub fn spec(image: &str) -> Command {
    let mut cmd = Command::new("flowctl");
    cmd.arg("api").arg("spec").arg("--image").arg(image);

    cmd
}

pub fn discovery(image: &str, config_path: &Path) -> Command {
    let mut cmd = Command::new("flowctl");
    cmd.arg("api")
        .arg("discover")
        .arg("--image")
        .arg(image)
        .arg("--config")
        .arg(config_path)
        .arg("--network")
        .arg(&settings().application.connector_network);

    cmd
}

#[cfg(all(test, feature = "flowctl"))]
mod test {
    use super::*;
    use crate::error::SubprocessError;
    use crate::services::subprocess::Subprocess;

    #[tokio::test]
    async fn connector_spec_works() {
        let mut cmd = spec("ghcr.io/estuary/source-hello-world:01fb856");
        let output = cmd.execute().await.expect("command output");
        assert_eq!(r#"{"type":"capture","#, &output[0..18]);
    }

    #[tokio::test]
    async fn connector_spec_fails_gracefully() {
        let mut cmd = spec("ghcr.io/estuary/source-hello-world:non-existant");
        assert!(matches!(
            cmd.execute().await.expect_err("connector should not exist"),
            SubprocessError::Failure { .. },
        ));
    }
}
