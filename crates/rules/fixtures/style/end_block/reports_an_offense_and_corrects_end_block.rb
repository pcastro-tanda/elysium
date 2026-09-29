END { test }
^^^ Avoid the use of `END` blocks. Use `Kernel#at_exit` instead.
