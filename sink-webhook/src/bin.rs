use apibara_core::starknet::v1alpha2::{Block, Filter};
use apibara_observability::init_opentelemetry;
use apibara_sink_common::{ConfigurationArgs, SinkConnector, SinkConnectorExt};
use apibara_sink_webhook::WebhookSink;
use clap::Parser;
use tokio_util::sync::CancellationToken;

#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Cli {
    /// The target url to send the request to.
    #[arg(long, env)]
    target_url: String,
    /// Additional headers to send with the request.
    #[arg(long, short = 'H', env, value_delimiter = ',')]
    header: Vec<String>,
    #[arg(long, env, action)]
    /// Send the data received from the transform step as is, this is useful for
    /// Discord/Telegram/... APIs
    raw: bool,
    #[command(flatten)]
    configuration: ConfigurationArgs,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    init_opentelemetry()?;
    let args = Cli::parse();

    let sink = WebhookSink::new(args.target_url, args.raw)?.with_headers(&args.header)?;
    let ct = CancellationToken::new();
    let connector = SinkConnector::<Filter, Block>::from_configuration_args(args.configuration)?;

    connector.consume_stream(sink, ct).await?;

    Ok(())
}
