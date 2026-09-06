use std::path::PathBuf;

use anyhow::Result;
use chromiumoxide::{Browser, BrowserConfig};
use futures::StreamExt;

pub async fn login(browser_path: &String, url: &String) -> Result<()> {
    let profile = profile_dir()?;
    std::fs::create_dir_all(&profile)?;

    // Start browser
    tracing::debug!("Configuring browser with path: {}", browser_path);
    let config = BrowserConfig::builder()
        .chrome_executable(browser_path)
        .user_data_dir(&profile)
        .with_head()
        .build()
        .map_err(|e| anyhow::anyhow!(e))?;
    tracing::debug!("Browser configuration created");

    tracing::debug!("Launching browser...");
    let (mut browser, mut handler) = Browser::launch(config).await?;
    let handle = tokio::spawn(async move { while handler.next().await.is_some() {} });
    tracing::debug!("Browser launched successfully");

    browser.new_page(url).await?;

    println!("Login in the browser and press Enter here");
    let mut buf = String::new();
    std::io::stdin().read_line(&mut buf)?;

    tracing::info!("Closing browser and saving profile");

    browser.close().await?;
    handle.await?;

    Ok(())
}

pub fn profile_dir() -> Result<PathBuf> {
    let home_dir =
        dirs::home_dir().ok_or_else(|| anyhow::anyhow!("Failed to find home directory"))?;

    Ok(home_dir.join("web2pdf").join("chrome-profile"))
}
