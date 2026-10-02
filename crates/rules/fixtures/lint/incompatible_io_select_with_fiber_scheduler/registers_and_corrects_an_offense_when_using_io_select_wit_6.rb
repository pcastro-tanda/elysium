IO.select([io], nil, nil, timeout)
^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^ Use `io.wait_readable(timeout)` instead of `IO.select([io], nil, nil, timeout)`.
