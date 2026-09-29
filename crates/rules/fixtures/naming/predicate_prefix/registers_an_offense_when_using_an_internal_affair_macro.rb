def_node_matcher :is_hello, <<~PATTERN
                 ^^^^^^^^^ Rename `is_hello` to `hello?`.
  (send
    (send nil? :method_name) :==
    (str 'hello'))
PATTERN
