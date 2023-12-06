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

run: 			## Run the debugger
	cargo run --bin dap-gui-ui -- $(RUN_ARGS)

.PHONY: repl
repl: python-develop ## Open ipython repl with debugger loaded
	ipython -c "import pythondap; d = pythondap.Debugger([4], file='./test.py')" -i

.PHOHY: python-develop
python-develop: ## Compile and install a development version of the debugger
	maturin develop --manifest-path pythondap/Cargo.toml
