IO.select(nil, [self], nil, timeout)
^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^ Use `self.wait_writable(timeout)` instead of `IO.select(nil, [self], nil, timeout)`.
