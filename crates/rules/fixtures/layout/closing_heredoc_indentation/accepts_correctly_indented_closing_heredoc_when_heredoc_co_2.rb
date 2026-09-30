def_node_matcher :eval_without_location?, <<~PATTERN
  {
    (send $(send _ $:sort ...) ${:[] :at :slice} {(int 0) (int -1)})

    (send $(send _ $:sort_by _) ${:last :first})
  }
PATTERN
