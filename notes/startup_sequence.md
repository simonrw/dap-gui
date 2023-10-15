# Startup sequence

1. Load persisted state (if present)
    * breakpoints
    * function breakpoints
    * exception breakpoints
    * project/files?
1. Send `Initialize`
    * `lines_starting_at_one = true`
1. Send `Launch` request (adapter specific options)

## Thoughts

* Add "clear all" button to breakpoints
* Do we want the debugger to remember previously opened files?
    * keep a persistant debugger instance open, probably not