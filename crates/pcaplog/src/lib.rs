use etherparse::{SlicedPacket, TransportSlice};
use eyre::WrapErr;
use pcap_file::pcapng::{PcapNgParser, blocks::enhanced_packet::EnhancedPacketBlock};
use std::{io::BufReader, path::Path};
use transport::{Message, Reader};

pub fn extract_messages(path: impl AsRef<Path>, port: u16) -> eyre::Result<Vec<Message>> {
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

            let mut i = 0;
            loop {
                tracing::trace!(packet = %i, "next packet");
                match pcap_parser.next_block(src) {
                    Ok((rem, block)) => {
                        tracing::trace!("got block");
                        match block {
                            pcap_file::pcapng::Block::EnhancedPacket(EnhancedPacketBlock {
                                data,
                                ..
                            }) => {
                                tracing::trace!("block length {}", data.len());
                                match SlicedPacket::from_ethernet(&data) {
                                    Ok(value) => {
                                        tracing::trace!("got sliced packet");
                                        if let Some(TransportSlice::Tcp(tcph)) = value.transport {
                                            tracing::trace!("got tcp layer");

                                            let payload = tcph.payload();
                                            if payload.is_empty() {
                                                tracing::trace!("no payload");
                                                src = rem;
                                                i += 1;
                                                continue;
                                            }
                                            // skip packets that are not for the specified port
                                            if tcph.source_port() != port
                                                && tcph.destination_port() != port
                                            {
                                                i += 1;
                                                src = rem;
                                                continue;
                                            }

                                            messages.extend_from_slice(payload);
                                        }
                                    }
                                    Err(e) => {
                                        tracing::warn!(error = %e, "error parsing package as ethernet frame");
                                        continue;
                                    }
                                }
                            }
                            e => tracing::warn!("unhandled block type {e:?}"),
                        }

                        src = rem;
                    }
                    Err(e) => {
                        tracing::error!(error = %e, "parsing next block");
                        break;
                    }
                }
                i += 1;
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
        Some(_) | None => eyre::bail!("invalid extension, expected .pcap or .pcapng"),
    };

    Ok(result)
}
