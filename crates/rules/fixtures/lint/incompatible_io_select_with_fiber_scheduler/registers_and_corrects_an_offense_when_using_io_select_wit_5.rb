::IO.select([io], [], [], timeout)
^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^ Use `io.wait_readable(timeout)` instead of `::IO.select([io], [], [], timeout)`.
