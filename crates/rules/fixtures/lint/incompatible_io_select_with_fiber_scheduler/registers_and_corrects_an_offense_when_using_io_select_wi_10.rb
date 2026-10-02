IO.select([self], nil, nil, timeout)
^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^ Use `self.wait_readable(timeout)` instead of `IO.select([self], nil, nil, timeout)`.
