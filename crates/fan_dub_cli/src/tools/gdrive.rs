// Config for the client secrets for the Google Drive API
use google_sheets4::{
    Error, FieldMask, Result, Sheets, api::ValueRange, hyper, hyper_rustls, hyper_util, yup_oauth2,
};
use serde::{Deserialize, Serialize};

type Connector = hyper_rustls::HttpsConnector<hyper_util::client::legacy::connect::HttpConnector>;

const CLIENT_SECRET: &str = include_str!("client_secret.json");

pub async fn basic_flow() -> anyhow::Result<()> {
    let secret = yup_oauth2::parse_application_secret(CLIENT_SECRET)?;
    let auth = yup_oauth2::InstalledFlowAuthenticator::builder(
        secret,
        yup_oauth2::InstalledFlowReturnMethod::HTTPRedirect,
    )
    .build()
    .await?;
    eprintln!("Starting Google Drive API flow...");
    let client: hyper_util::client::legacy::Client<Connector, _> =
        hyper_util::client::legacy::Client::builder(hyper_util::rt::TokioExecutor::new()).build(
            hyper_rustls::HttpsConnectorBuilder::new()
                .with_native_roots()?
                .https_or_http()
                .enable_http1()
                .build(),
        );
    let hub = Sheets::new(client, auth);

    let result = hub.spreadsheets()
        .get("1jCy594_CfzqbtnOh9SSYTEVnypnLqpGG_tTyr18Yu9Y")
        .doit()
        .await?;
    eprintln!("Reponse: {:?}", result);
    Ok(())
}
