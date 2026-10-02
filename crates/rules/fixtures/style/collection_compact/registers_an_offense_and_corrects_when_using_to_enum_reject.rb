array.to_enum.reject { |e| e.nil? }
              ^^^^^^^^^^^^^^^^^^^^^ Use `compact` instead of `reject { |e| e.nil? }`.
array.to_enum.reject! { |e| e.nil? }
              ^^^^^^^^^^^^^^^^^^^^^^ Use `compact!` instead of `reject! { |e| e.nil? }`.
