use std::io::IsTerminal;

use axum::body::Body;
use axum::http::Request;
use tower_governor::key_extractor::SmartIpKeyExtractor;
use tracing::Span;
use tracing_appender::non_blocking::WorkerGuard;
use tracing_subscriber::layer::SubscriberExt;
use tracing_subscriber::util::SubscriberInitExt;
use tracing_subscriber::{filter, Layer};

use crate::config::{TracingFormat, TracingSettings};
use tower_governor::key_extractor::KeyExtractor;

pub(crate) fn init(tracing_settings: TracingSettings) -> Option<WorkerGuard> {
    let app_name = env!("CARGO_PKG_NAME").replace('-', "_");
    let app_version = env!("CARGO_PKG_VERSION");

    let stdout_default_level: tracing::Level = tracing_settings.stdout_default_level.clone().into();
    let stdout_level: tracing::Level = tracing_settings.stdout_level.clone().into();
    let log_default_level: tracing::Level = tracing_settings.log_level.clone().into();
    let log_level: tracing::Level = tracing_settings.log_level.clone().into();

    let (file_layer, guard) = match tracing_settings.log_enabled {
        true => {
            match std::fs::metadata(tracing_settings.log_dir.as_str()) {
                Ok(md) => {
                    if !md.is_dir() {
                        panic!("{} is not a directory", tracing_settings.log_dir.as_str());
                    }

                    if md.permissions().readonly() {
                        panic!("{} is not writable", tracing_settings.log_dir.as_str());
                    }
                }
                Err(_) => {
                    panic!(
                        "Logs path {} does not exists",
                        tracing_settings.log_dir.as_str()
                    )
                }
            }

            let file_appender = tracing_appender::rolling::daily(
                tracing_settings.log_dir,
                format!("{app_name}-{app_version}"),
            );
            let (file_writer, guard) = tracing_appender::non_blocking(file_appender);

            let file_layer = tracing_subscriber::fmt::Layer::default()
                .with_ansi(false)
                .with_target(tracing_settings.log_target)
                .with_writer(file_writer);

            (
                Some(
                    match tracing_settings.log_format {
                        TracingFormat::Compact => file_layer.compact().boxed(),
                        TracingFormat::Pretty => file_layer.pretty().boxed(),
                        TracingFormat::Json => file_layer.json().boxed(),
                    }
                    .with_filter(
                        filter::Targets::new()
                            .with_default(log_default_level)
                            .with_target("tower_http::trace", tracing::Level::INFO)
                            .with_target(app_name.clone(), log_level),
                    ),
                ),
                Some(guard),
            )
        }
        false => (None, None),
    };

    let stdout_layer = match tracing_settings.stdout_enabled {
        true => {
            let stdout_layer = tracing_subscriber::fmt::layer()
                .with_ansi(std::io::stdout().is_terminal())
                .with_target(tracing_settings.stdout_target);

            Some(
                match tracing_settings.stdout_format {
                    TracingFormat::Compact => stdout_layer.compact().boxed(),
                    TracingFormat::Pretty => stdout_layer.pretty().boxed(),
                    TracingFormat::Json => stdout_layer.json().boxed(),
                }
                .with_filter(
                    filter::Targets::new()
                        .with_default(stdout_default_level)
                        .with_target("tower_http::trace", tracing::Level::INFO)
                        .with_target(app_name, stdout_level),
                ),
            )
        }
        false => None,
    };

    tracing_subscriber::registry()
        .with(file_layer)
        .with(stdout_layer)
        .init();

    guard
}

pub(crate) fn make_span_with(request: &Request<Body>) -> Span {
    tracing::info_span!("request",
        app_name = env!("CARGO_PKG_NAME"),
        app_version = env!("CARGO_PKG_VERSION"),
        hostname = hostname::get()
                .unwrap_or(std::ffi::OsString::from("<uknown>"))
                .into_string()
                .unwrap_or(String::from("<unknown>")),

        request_id = uuid::Uuid::new_v4().hyphenated().encode_lower(&mut uuid::Uuid::encode_buffer()),
        method = %request.method(),
        uri = %request.uri(),
        client_ip = SmartIpKeyExtractor.extract(request)
        .map_or_else(|_| tracing::field::display(String::from("<unknown>")), |ip| tracing::field::display(ip.to_string())),
        user_agent = match request.headers().get("User-Agent").map(|h| h.as_bytes()) {
            None => "<unknown>",
            Some(h) => std::str::from_utf8(h).unwrap_or("<unknown>"),
        },

        // Fields must be defined to be used, define them as empty if they populate later
        status = tracing::field::Empty,
        latency = tracing::field::Empty,
        user_id = tracing::field::Empty,
    )
}
