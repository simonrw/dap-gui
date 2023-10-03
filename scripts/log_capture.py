#!/usr/bin/env python

import argparse
import json

from scapy.all import rdpcap


if __name__ == "__main__":
    parser = argparse.ArgumentParser()
    parser.add_argument("file")
    parser.add_argument("-o", "--output", type=argparse.FileType("w"), default="-")
    args = parser.parse_args()

    a = rdpcap(args.file)

    combined = b""
    pair: set[tuple[str, int, str, int]] | None = None

    for packet in a:
        if not hasattr(packet, "load"):
            continue

        if not pair and b"Content-Length" in packet.load:
            pair = set(
                [
                    (packet.src, packet.sport, packet.dst, packet.dport),
                    (packet.dst, packet.dport, packet.src, packet.sport),
                ]
            )
            combined += packet.load

        if not pair:
            continue

        if (packet.src, packet.sport, packet.dst, packet.dport) not in pair:
            continue

        combined += packet.load

    messages = []
    # parse stream
    while combined:
        if not combined.startswith(b"Content-Length"):
            raise RuntimeError(f"invalid remaining content: {combined.decode()}")

        combined = combined.lstrip(b"Content-Length:")
        combined = combined.lstrip(b" ")

        length = ""
        while True:
            if combined[0] < ord("0") or combined[0] > ord("9"):
                break
            length += chr(combined[0])
            combined = combined[1:]

        content_length = int(length)
        # strip \r\n\r\n
        combined = combined[4:]


        json_str = combined[:content_length]
        combined = combined[content_length:]

        messages.append(json.loads(json_str.decode()))

    json.dump(messages, args.output, indent=2)
