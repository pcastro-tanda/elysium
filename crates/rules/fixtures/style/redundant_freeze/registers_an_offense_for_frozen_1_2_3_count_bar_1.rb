CONST = [1, 2, 3].count { bar?(_1) }.freeze
        ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^ Do not freeze immutable objects, as freezing them has no effect.
