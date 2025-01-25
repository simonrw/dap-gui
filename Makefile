SHELL := bash
PORT ?= 5678

# arguments for steps
RUN_ARGS ?=

help:           ## Show this help.
	@fgrep -h "##" $(MAKEFILE_LIST) | fgrep -v fgrep | sed -e 's/\\$$//' | sed -e 's/##//'

run-server:     ## Run the DAP server
	@echo "!!! Running DAP server on port ${PORT}"
	python -m debugpy.adapter --host 127.0.0.1 --port ${PORT} --log-stderr

run-attach:
	@echo "!!! Running attachable script on port ${PORT}"
	python attach.py

run: 			## Run the debugger
	cargo run --bin dap-gui-ui -- $(RUN_ARGS)

.PHONY: pyrepl
pyrepl: python-develop ## Open ipython repl with debugger loaded
	pythondap -b 9 -f ./attach.py launch_configuration/testdata/vscode/localstack.code-workspace -n "Remote Attach (ext)"

.PHOHY: python-develop
python-develop: ## Compile and install a development version of the debugger
	maturin develop --manifest-path pythondap/Cargo.toml

.PHONY: tui-attach
tui-attach:
	cargo r -p tui -- launch.json -n Attach -b attach.py:29

.PHONY: tui-launch
tui-launch:
	cargo r -p tui -- launch.json -n Launch -b test.py:4

.PHONY: repl-attach
repl-attach:
	cargo r -p repl -- launch.json -n Attach -b attach.py:29

.PHONY: repl-launch
repl-launch:
	cargo r -p repl -- launch.json -n Launch -b test.py:4
