# Debugger interface

*What is the interface of the debugger, wrapping the transport layer?*

Should be driven by requirements from the GUI, i.e. support

* notifications on state change
* ui triggering:
    * breakpoint setting
    * variable inspection
* streaming output from the debugee


We should group the events into logical phases, and name them concretely:

## Stages

* `initialise`: load previous debugger state for a project, send `initialize` event, set previous breakpoints, launch the program
* `running`: output logs from the process
* `paused`: show stack frame information, variables etc., indicate stopped position
