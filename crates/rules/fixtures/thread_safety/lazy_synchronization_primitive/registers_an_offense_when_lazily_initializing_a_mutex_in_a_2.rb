def mutex = @mutex ||= Mutex.new
            ^^^^^^^^^^^^^^^^^^^^ Do not lazily initialize synchronization primitives with `||=`.
