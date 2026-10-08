n.times.map { |i| i.to_s }
^^^^^^^^^^^^^^^^^^^^^^^^^^ Use `Array.new(n)` with a block instead of `.times.map` only if `n` is always 0 or more.
