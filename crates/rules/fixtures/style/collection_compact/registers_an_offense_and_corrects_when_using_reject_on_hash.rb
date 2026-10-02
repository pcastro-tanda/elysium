hash.reject { |k, v| v.nil? }
     ^^^^^^^^^^^^^^^^^^^^^^^^ Use `compact` instead of `reject { |k, v| v.nil? }`.
hash.reject! { |k, v| v.nil? }
     ^^^^^^^^^^^^^^^^^^^^^^^^^ Use `compact!` instead of `reject! { |k, v| v.nil? }`.
