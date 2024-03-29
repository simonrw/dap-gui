# DAP GUI

[![test](https://github.com/simonrw/dap-gui/actions/workflows/test.yml/badge.svg)](https://github.com/simonrw/dap-gui/actions/workflows/test.yml)

Very early prototype of a general purpose GUI debugger based on the [DAP][dap] protocol and [egui]([url](https://github.com/emilk/egui)).

https://github.com/simonrw/dap-gui/assets/59756/a31d6a97-431f-48b7-a051-baac3ca9b167



## Motivation

This repository exists because I do not like any of the debuggers I (currently) use day to day. I have tried:

* pycharm
* vs code
* neovim dap

They do not feel right to me.

My hope with this project is to create a general purpose GUI on top of the [DAP][dap] protocol that I like to use.

## Features

* fast
* stable
* TBD

## Internals

WIP

### Code layout and architecture

* `transport` crate:
  * serialisation and deserialisation of wire protocol
  * send messages with and without responses
  * publish received events
* `debugger` crate:
  * high level controls like `continue`
  * breakpoint management
  * initialisation of debugger state
* `server` crate:
  * abstraction over running DAP servers
* `pcaplog` crate:
  * print messages from pcap(ng) captures (prototype)
* `gui` crate:
  * main GUI implementation using `egui`/`eframe`
* `state` crate:
  * handles cross-session state persistence
* `launch_configuration` crate:
  * represents different launch configration options, e.g. `vscode` `launch.json` files

### States and transitions

The diagram below represents the different state transitions used by the [`debugger::Debugger`](./debugger/src/debugger.rs) type.

```mermaid
---
title: Debugger states
---

stateDiagram-v2
    [*] --> Initialized: [1]
    Initialized --> Running: [2]
    Running --> Paused: [2]
    Paused --> Running: [3]
    Paused --> ScopeChange: [4]
    ScopeChange --> Paused
    Running --> Terminated: [5]
    Terminated --> [*]
```

[dap]: https://microsoft.github.io/debug-adapter-protocol/

