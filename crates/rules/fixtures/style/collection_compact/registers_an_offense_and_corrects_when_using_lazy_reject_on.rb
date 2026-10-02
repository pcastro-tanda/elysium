array.lazy.reject { |e| e.nil? }
           ^^^^^^^^^^^^^^^^^^^^^ Use `compact` instead of `reject { |e| e.nil? }`.
array.lazy.reject! { |e| e.nil? }
           ^^^^^^^^^^^^^^^^^^^^^^ Use `compact!` instead of `reject! { |e| e.nil? }`.
