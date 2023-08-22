#!/usr/bin/env python

import asyncio
import json
import copy
import argparse

from pygments.lexers.javascript import JavascriptLexer
from prompt_toolkit import PromptSession
from prompt_toolkit.history import FileHistory
from prompt_toolkit.auto_suggest import AutoSuggestFromHistory
from prompt_toolkit.patch_stdout import patch_stdout
from prompt_toolkit.lexers import PygmentsLexer

import pygments
from prompt_toolkit.formatted_text import PygmentsTokens
from prompt_toolkit import print_formatted_text


SEQ = 0


def next_seq() -> int:
    global SEQ
    value = copy.copy(SEQ)
    SEQ += 1
    return value


def encode(msg: str) -> str:
    # keep track of sequence number so the user doesn't have to
    decoded = json.loads(msg)
    decoded["seq"] = next_seq()
    decoded["type"] = "request"
    encoded = json.dumps(decoded)

    content_length = len(encoded.encode())
    return f"Content-Length: {content_length}\r\n\r\n{encoded}"


def print_highlighted(text: str):
    # strip headers
    for line in text.splitlines():
        if not line.strip():
            continue
        if "content-length:" in line.lower():
            continue

        # Printing the output of a pygments lexer.
        tokens = list(pygments.lex(line, lexer=JavascriptLexer()))
        print_formatted_text(PygmentsTokens(tokens))


async def print_loop(reader: asyncio.StreamReader):
    while True:
        data = await reader.read(1024)
        print_highlighted(data.decode())


async def async_input(session: PromptSession) -> str:
    return await session.prompt_async("> ", lexer=PygmentsLexer(JavascriptLexer))


async def main(port: int):
    reader, writer = await asyncio.open_connection("127.0.0.1", port)
    handle = asyncio.create_task(print_loop(reader))

    session = PromptSession(
        history=FileHistory("/tmp/dap-repl.log"), auto_suggest=AutoSuggestFromHistory()
    )
    try:
        while True:
            with patch_stdout():
                raw_input = await async_input(session)

            # handle early quitting
            if raw_input.strip() == "q":
                handle.cancel()
                break

            msg = encode(raw_input)
            print(f"Sending message: {msg}")
            writer.write(msg.encode())
            await writer.drain()
    except KeyboardInterrupt:
        handle.cancel()


if __name__ == "__main__":
    parser = argparse.ArgumentParser()
    parser.add_argument("-p", "--port", type=int, required=True)
    args = parser.parse_args()

    loop = asyncio.get_event_loop()
    loop.run_until_complete(main(args.port))
