eval('puts 42',
     binding,
     __FILE__,
     __LINE__)
     ^^^^^^^^ Incorrect line number for `eval`; use `__LINE__ - 3` instead of `__LINE__`.
