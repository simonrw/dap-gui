use anyhow::Context;
use etherparse::SlicedPacket;
use pcap_file::pcapng::{blocks::enhanced_packet::EnhancedPacketBlock, PcapNgParser};
use std::{io::BufReader, path::Path};
use transport::{Message, Reader};

pub fn extract_messages(path: impl AsRef<Path>) -> anyhow::Result<Vec<Message>> {
    let path = path.as_ref();

    // TODO: not great for memory usage or DOS...
    tracing::debug!("reading file into memory");
    let file_bytes = std::fs::read(path).context("reading file bytes")?;

    let mut result = Vec::new();
    match path.extension().and_then(|s| s.to_str()) {
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
                        if let pcap_file::pcapng::Block::EnhancedPacket(EnhancedPacketBlock {
                            data,
                            ..
                        }) = block
                        {
                            match SlicedPacket::from_ethernet(&data) {
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
                            }
                        }

                        src = rem;
                    }
                    Err(e) => {
                        tracing::error!(error = %e, "parsing next block");
                        break;
                    }
                }
            }

            let mut reader = transport::reader::get(BufReader::new(messages.as_slice()));
            loop {
                match reader.poll_message() {
                    Ok(Some(message)) => {
                        result.push(message);
                    }
                    Ok(None) => break,
                    Err(e) => tracing::warn!(error = ?e, "invalid message"),
                }
            }
        }
        Some(_) | None => anyhow::bail!("invalid extension, expected .pcap or .pcapng"),
    };

    Ok(result)
}
