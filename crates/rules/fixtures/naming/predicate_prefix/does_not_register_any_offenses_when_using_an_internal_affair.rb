def_node_matcher :is_hello, <<~PATTERN
  (send
    (send nil? :method_name) :==
    (str 'hello'))
PATTERN
