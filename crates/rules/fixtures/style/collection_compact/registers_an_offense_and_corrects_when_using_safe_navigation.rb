array&.reject { |e| e&.nil? }
       ^^^^^^^^^^^^^^^^^^^^^^ Use `compact` instead of `reject { |e| e&.nil? }`.
array&.reject! { |e| e&.nil? }
       ^^^^^^^^^^^^^^^^^^^^^^^ Use `compact!` instead of `reject! { |e| e&.nil? }`.
