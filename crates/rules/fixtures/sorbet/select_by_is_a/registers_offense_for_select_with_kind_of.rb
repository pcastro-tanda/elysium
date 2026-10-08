strings_or_integers.select { |e| e.kind_of?(String) }
^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^ Use `grep` instead of `select` when using it only for type narrowing.
