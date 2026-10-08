_, ws = IO.select([], [wp])
        ^^^^^^^^^^^^^^^^^^^ Use `wp.wait_writable` instead of `IO.select([], [wp])`.
