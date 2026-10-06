array.collect(&:count).sum(10)
      ^^^^^^^^^^^^^^^^^^^^^^^^ Use `sum(10) { ... }` instead of `collect { ... }.sum(10)`.
