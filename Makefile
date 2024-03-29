SHELL := bash
PORT ?= 5678

# arguments for steps
RUN_ARGS ?=

help:           ## Show this help.
	@fgrep -h "##" $(MAKEFILE_LIST) | fgrep -v fgrep | sed -e 's/\\$$//' | sed -e 's/##//'

run-server:     ## Run the DAP server
	@echo "!!! Running DAP server on port ${PORT}"
	@while true; \
		do python -m debugpy.adapter --host 127.0.0.1 --port ${PORT} --log-stderr; \
	done

run-attach:
	@echo "!!! Running attachable script on port ${PORT}"
	@while true; \
		do python attach.py; \
	done

run: 			## Run the debugger
	cargo run --bin dap-gui-ui -- $(RUN_ARGS)
