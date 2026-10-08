array.collect { |elem| elem ** 2 }.sum(10)
      ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^ Use `sum(10) { ... }` instead of `collect { ... }.sum(10)`.
