puts 1 if !foo.select! { |e| !e.bar? }
           ^^^^^^^^^^^^^^^^^^^^^^^^^^^ Use `reject!` instead of inverting `select!`.
