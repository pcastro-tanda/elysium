foo[1] ||= collection
           .map(&:do_something).compact
            ^^^^^^^^^^^^^^^^^^^^^^^^^^^ Use `filter_map` instead.
