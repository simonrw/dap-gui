SHELL := bash

# arguments for steps
RUN_ARGS ?=

help:           ## Show this help.
	@fgrep -h "##" $(MAKEFILE_LIST) | fgrep -v fgrep | sed -e 's/\\$$//' | sed -e 's/##//'

run-server:     ## Run the DAP server
	@echo "!!! Running DAP server on port 5678"
	@while true; \
		do python -m debugpy.adapter --host 127.0.0.1 --port 5678 --log-stderr; \
	done

run: 			## Run the debugger
	cargo run --bin dap-gui-ui -- $(RUN_ARGS)
