use std::{io::BufReader, path::PathBuf};

use anyhow::Context;
use clap::Parser;
use etherparse::SlicedPacket;
use pcap_file::pcapng::{blocks::enhanced_packet::EnhancedPacketBlock, PcapNgParser};
use tracing_subscriber::EnvFilter;
use transport::Reader;

#[derive(Debug, Parser)]
struct Args {
    file: PathBuf,
}

fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::from_default_env())
        .init();

    let args = Args::parse();
    tracing::debug!(?args, "parsed command line arguments");

    // TODO: not great for memory usage or DOS...
    tracing::debug!("reading file into memory");
    let file_bytes = std::fs::read(&args.file).context("reading file bytes")?;

    match args.file.extension().and_then(|s| s.to_str()) {
        Some("pcap") => todo!(),
        Some("pcapng") => {
            tracing::debug!("parsing file");

            let mut src = &file_bytes[..];

            let (rem, mut pcap_parser) = PcapNgParser::new(src).context("parsing file")?;
            src = rem;

            let mut messages = Vec::new();

            loop {
                match pcap_parser.next_block(src) {
                    Ok((rem, block)) => {
                        match block {
                            pcap_file::pcapng::Block::EnhancedPacket(EnhancedPacketBlock {
                                data,
                                ..
                            }) => match SlicedPacket::from_ethernet(&data) {
                                Ok(value) => {
                                    let payload = value.payload;
                                    if payload.is_empty() {
                                        src = rem;
                                        continue;
                                    }

                                    messages.extend_from_slice(payload);
                                }
                                Err(e) => {
                                    tracing::warn!(error = %e, "error parsing package as ethernet frame");
                                    continue;
                                }
                            },
                            _ => {}
                        }

                        src = rem;
                    }
                    Err(e) => {
                        tracing::error!(error = %e, "parsing next block");
                        break;
                    }
                }
            }

            let mut reader = Reader::new(BufReader::new(messages.as_slice()));
            loop {
                match reader.poll_message() {
                    Ok(Some(message)) => {
                        let serialized =
                            serde_json::to_string(&message).context("serialising message")?;
                        println!("{serialized}");
                    }
                    Ok(None) => break,
                    Err(e) => tracing::warn!(error = ?e, "invalid message"),
                }
            }
        }
        Some(_) | None => anyhow::bail!("invalid extension, expected .pcap or .pcapng"),
    };

    Ok(())
}
