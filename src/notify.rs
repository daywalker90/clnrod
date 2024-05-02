use std::time::Duration;

use anyhow::{anyhow, Error};
use lettre::{
    message::header::ContentType,
    transport::smtp::{
        authentication::Credentials,
        client::{Tls, TlsParameters},
    },
    AsyncSmtpTransport, AsyncTransport, Message, Tokio1Executor,
};
use log::{info, warn};

use crate::structs::{Config, NotifyVerbosity};

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
        .from(config.email_from.value.parse().unwrap())
        .to(config.email_to.value.parse().unwrap())
        .subject(subject.clone())
        .header(header)
        .body(body.to_string())
        .unwrap();

    let creds = Credentials::new(
        config.smtp_username.value.clone(),
        config.smtp_password.value.clone(),
    );

    let tls_parameters = TlsParameters::builder(config.smtp_server.value.clone())
        .dangerous_accept_invalid_certs(false)
        .build_rustls()?;

    let mailer = AsyncSmtpTransport::<Tokio1Executor>::starttls_relay(&config.smtp_server.value)?
        .credentials(creds)
        .tls(Tls::Required(tls_parameters))
        .port(config.smtp_port.value)
        .timeout(Some(Duration::from_secs(60)))
        .build();

    // Send the email
    let result = mailer.send(email).await;
    if result.is_ok() {
        info!(
            "Sent email with subject: `{}` to: `{}`",
            subject, config.email_to.value
        );
        Ok(())
    } else {
        Err(anyhow!("Failed to send email: {:?}", result))
    }
}

pub async fn notify(
    config: &Config,
    subject: &str,
    reason: &str,
    pubkey: String,
    verbosity: NotifyVerbosity,
) {
    if verbosity == NotifyVerbosity::Error {
        warn!("{}: pubkey: {} reason: {}", subject, pubkey, reason);
    } else {
        info!("{}: pubkey: {} reason: {}", subject, pubkey, reason);
    }

    if config.send_mail && config.notify_verbosity.value >= verbosity {
        if let Err(e) = send_mail(
            config,
            &subject.to_string(),
            &format!("pubkey:\n{}\n\nReason:\n{}", pubkey, reason),
            false,
        )
        .await
        {
            warn!("Error sending mail: {} pubkey: {}", e, pubkey);
        };
    }
}
