use postgresql_archive::configuration::theseus;
use postgresql_archive::extract;
use postgresql_archive::{get_archive, get_version};
use semver::VersionReq;
use std::fs::remove_dir_all;
use test_log::test;

#[test(tokio::test)]
async fn test_get_version_not_found() -> postgresql_archive::Result<()> {
    let invalid_version_req = VersionReq::parse("=1.0.0")?;
    let result = get_version(theseus::URL, &invalid_version_req).await;

    assert!(result.is_err());
    Ok(())
}

#[test(tokio::test)]
async fn test_get_version() -> anyhow::Result<()> {
    let version_req = VersionReq::parse("=16.3.0")?;
    let latest_version = get_version(theseus::URL, &version_req).await?;

    assert!(version_req.matches(&latest_version));
    Ok(())
}

#[test(tokio::test)]
async fn test_get_archive_and_extract() -> anyhow::Result<()> {
    let url = theseus::URL;
    let version_req = VersionReq::parse("=16.3.0")?;
    let (archive_version, archive) = get_archive(url, &version_req).await?;

    assert!(version_req.matches(&archive_version));

    let out_dir = tempfile::tempdir()?.path().to_path_buf();
    let files = extract(url, &archive, &out_dir).await?;
    #[cfg(all(target_os = "linux", target_arch = "x86_64"))]
    assert_eq!(1_312, files.len());
    #[cfg(all(target_os = "macos", target_arch = "aarch64"))]
    assert_eq!(1_271, files.len());
    #[cfg(all(target_os = "macos", target_arch = "x86_64"))]
    assert_eq!(1_271, files.len());
    #[cfg(all(target_os = "windows", target_arch = "x86_64"))]
    assert_eq!(3_120, files.len());
    remove_dir_all(&out_dir)?;
    Ok(())
}

#[test(tokio::test)]
async fn test_get_archive_version_not_found() -> postgresql_archive::Result<()> {
    let invalid_version_req = VersionReq::parse("=1.0.0")?;
    let result = get_archive(theseus::URL, &invalid_version_req).await;

    assert!(result.is_err());
    Ok(())
}
