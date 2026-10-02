rs, _ = IO.select([rp], [])
        ^^^^^^^^^^^^^^^^^^^ Use `rp.wait_readable` instead of `IO.select([rp], [])`.
