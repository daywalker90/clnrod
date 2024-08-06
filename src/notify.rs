use std::time::Duration;

use anyhow::{anyhow, Error};
use cln_plugin::Plugin;
use cln_rpc::primitives::PublicKey;
use lettre::{
    message::header::ContentType,
    transport::smtp::{
        authentication::Credentials,
        client::{Tls, TlsParameters},
    },
    AsyncSmtpTransport, AsyncTransport, Message, Tokio1Executor,
};
use log::{info, warn};

use crate::structs::{Config, NotifyVerbosity, PluginState};

async fn send_mail(
    config: &Config,
    subject: &String,
    body: &String,
    html: bool,
) -> Result<(), Error> {
    let header = if html {
        ContentType::TEXT_HTML
    } else {
        ContentType::TEXT_PLAIN
    };

    let email = Message::builder()
        .from(config.email_from.parse().unwrap())
        .to(config.email_to.parse().unwrap())
        .subject(subject.clone())
        .header(header)
        .body(body.to_string())
        .unwrap();

    let creds = Credentials::new(config.smtp_username.clone(), config.smtp_password.clone());

    let tls_parameters = TlsParameters::builder(config.smtp_server.clone())
        .dangerous_accept_invalid_certs(false)
        .build_rustls()?;

    let mailer = AsyncSmtpTransport::<Tokio1Executor>::starttls_relay(&config.smtp_server)?
        .credentials(creds)
        .tls(Tls::Required(tls_parameters))
        .port(config.smtp_port)
        .timeout(Some(Duration::from_secs(60)))
        .build();

    // Send the email
    let result = mailer.send(email).await;
    if result.is_ok() {
        info!(
            "Sent email with subject: `{}` to: `{}`",
            subject, config.email_to
        );
        Ok(())
    } else {
        Err(anyhow!("Failed to send email: {:?}", result))
    }
}

pub async fn notify(
    plugin: &Plugin<PluginState>,
    subject: &str,
    body: &str,
    pubkey: Option<PublicKey>,
    verbosity: NotifyVerbosity,
) {
    let config = plugin.state().config.lock().clone();
    let cache = if let Some(pk) = &pubkey {
        plugin.state().peerdata_cache.lock().get(pk).cloned()
    } else {
        None
    };
    if verbosity == NotifyVerbosity::Error {
        warn!(
            "{}: pubkey: {} message: {}",
            subject,
            pubkey
                .map(|pk| pk.to_string())
                .unwrap_or("None".to_string()),
            body
        );
    } else {
        info!(
            "{}: pubkey: {} message: {}",
            subject,
            pubkey
                .map(|pk| pk.to_string())
                .unwrap_or("None".to_string()),
            body
        );
    }

    if config.send_mail && config.notify_verbosity >= verbosity {
        if let Err(e) = send_mail(
            &config,
            &subject.to_string(),
            &format!(
                "pubkey:\n{}\n\nMessage:\n{}\n\nCollected data:\n{}",
                pubkey
                    .map(|pk| pk.to_string())
                    .unwrap_or("None".to_string()),
                body,
                cache.map(|pd| pd.to_string()).unwrap_or("None".to_string())
            ),
            false,
        )
        .await
        {
            warn!("Error sending mail: {} pubkey: {:?}", e, pubkey);
        };
    }
}
