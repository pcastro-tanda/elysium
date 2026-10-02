::IO.select([io], [], [])
^^^^^^^^^^^^^^^^^^^^^^^^^ Use `io.wait_readable` instead of `::IO.select([io], [], [])`.
