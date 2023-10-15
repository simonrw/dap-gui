# Startup sequence

1. Load persisted state (if present)
    * breakpoints
    * function breakpoints
    * exception breakpoints
    * project/files?
1. Send `Initialize`
    * `lines_starting_at_one = true`
1. Send `Launch` request (adapter specific options)
1. Wait for `Initialized` event to indicate adapter ready to accept breakpoint configuration
1. Configure pre-existing breakpoints
1. Send `ConfigurationDone` to indicate that the pre-configured breakpoints have been sent
1. Wait for `Stopped` event

## Thoughts

* Add "clear all" button to breakpoints
* Do we want the debugger to remember previously opened files?
    * keep a persistent debugger instance open, probably not
* Is Launch -> ConfigurationDone standard, or specific to debugpy?
