IO.select(nil, [io], nil, timeout)
^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^ Use `io.wait_writable(timeout)` instead of `IO.select(nil, [io], nil, timeout)`.
