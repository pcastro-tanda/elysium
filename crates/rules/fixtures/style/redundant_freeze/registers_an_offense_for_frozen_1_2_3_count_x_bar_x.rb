CONST = [1, 2, 3].count { |x| bar?(x) }.freeze
        ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^ Do not freeze immutable objects, as freezing them has no effect.
