n.times.collect { |i| i.to_s }
^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^ Use `Array.new(n)` with a block instead of `.times.collect` only if `n` is always 0 or more.
