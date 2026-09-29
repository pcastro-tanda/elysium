puts 1 if !foo.reject! { |e| !e.bar? }
           ^^^^^^^^^^^^^^^^^^^^^^^^^^^ Use `select!` instead of inverting `reject!`.
