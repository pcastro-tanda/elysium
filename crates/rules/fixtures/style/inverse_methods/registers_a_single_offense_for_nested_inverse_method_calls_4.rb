y.reject! { |key, _value| !(key =~ /c\d/) }
^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^ Use `select!` instead of inverting `reject!`.
