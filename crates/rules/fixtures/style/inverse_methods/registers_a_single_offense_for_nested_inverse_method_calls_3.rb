y.select! { |key, _value| !(key =~ /c\d/) }
^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^ Use `reject!` instead of inverting `select!`.
